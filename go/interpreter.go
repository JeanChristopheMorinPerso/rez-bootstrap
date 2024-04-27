package main

import (
	"archive/tar"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"sort"
	"strings"

	"github.com/klauspost/compress/zstd"
	"golang.org/x/sync/errgroup"
)

func GetInterpreters(release GitHubRelease) ([]Interpreter, error) {
	groups := map[string][]Interpreter{}
	installOnlyInterpreters := []Interpreter{}

	for _, asset := range release.Assets {
		if !strings.HasSuffix(asset.Name, ".tar.zst") && !strings.HasSuffix(asset.Name, ".tar.gz") {
			continue
		}

		interpreter, err := parseAsset(asset)
		if err != nil {
			return installOnlyInterpreters, fmt.Errorf("failed to parse asset %s: %w", asset.Name, err)
		}

		switch interpreter.Flavor {
		case FlavorInstallOnly:
			installOnlyInterpreters = append(installOnlyInterpreters, interpreter)
		case FlavorFull:
			key := fmt.Sprintf("%s-%s+%s-%s", interpreter.Implementation, interpreter.PythonVersion, interpreter.GitHubRelease, interpreter.Triple)
			groups[key] = append(groups[key], interpreter)
		}
	}

	// https://pkg.go.dev/golang.org/x/sync/errgroup#example-Group-Pipeline.
	errGroup, ctx := errgroup.WithContext(context.Background())

	inputChannel := make(chan Interpreter)
	errGroup.Go(func() error {
		defer close(inputChannel)

		for _, interpreter := range installOnlyInterpreters {
			if err := ctx.Err(); err != nil {
				return err
			}

			groupKey := fmt.Sprintf("%s-%s+%s-%s", interpreter.Implementation, interpreter.PythonVersion, interpreter.GitHubRelease, interpreter.Triple)
			group := groups[groupKey]

			// Sort by config. The first item will be the best match.
			sort.Sort(ByConfig(group))

			bestMatch := group[0]

			interpreter.InterpreterImplemented = &bestMatch

			select {
			case inputChannel <- bestMatch:
				continue
			case <-ctx.Done():
				return ctx.Err()
			}
		}
		return nil
	})

	// Start a fixed number of goroutines to get the PYTHON.json files.
	outputChannel := make(chan Interpreter)
	for i := 0; i < 10; i++ {
		errGroup.Go(func() error {
			for interpreter := range inputChannel {
				info, err := GetPythonInfo(interpreter.Url)
				if err != nil {
					return fmt.Errorf("failed to get python info for %s: %w", interpreter.Url, err)
				}

				interpreter.Info = info
				select {
				case outputChannel <- interpreter:
				case <-ctx.Done():
					return ctx.Err()
				}
			}
			return nil
		})
	}

	go func() {
		errGroup.Wait()
		close(outputChannel)
	}()

	interpreters := []Interpreter{}
	for interpreter := range outputChannel {
		interpreters = append(interpreters, interpreter)
	}

	if err := errGroup.Wait(); err != nil {
		return nil, fmt.Errorf("errgroup error: %w", err)
	}

	return interpreters, nil
}

func GetPythonInfo(url string) (PythonJSON, error) {
	var pythonJSON PythonJSON

	response, err := http.Get(url)
	if err != nil {
		return pythonJSON, fmt.Errorf("failed to query %q: %w", url, err)
	}

	defer response.Body.Close()

	decoder, err := zstd.NewReader(response.Body)
	if err != nil {
		return pythonJSON, fmt.Errorf("failed not create zstd reader for %q: %w", url, err)
	}

	defer decoder.Close()

	reader := tar.NewReader(decoder)
	for {
		header, err := reader.Next()
		if err == io.EOF {
			break
		}

		if err != nil {
			return pythonJSON, fmt.Errorf("failed to read tar file %q: %w", url, err)
		}

		if header.Name == "python/PYTHON.json" {
			err := json.NewDecoder(reader).Decode(&pythonJSON)

			if err != nil {
				return pythonJSON, fmt.Errorf("failed to decode python/PYTHON.json for %q: %w", url, err)
			}

			return pythonJSON, nil
		}
	}

	return pythonJSON, fmt.Errorf("could not find python/PYTHON.json in %q", url)
}
