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

type InterpreterFlavor int

const (
	FlavorFull InterpreterFlavor = iota
	FlavorInstallOnly
)

type PythonJSON struct {
	AppleSDKDeploymentTarget string   `json:"apple_sdk_deployment_target"`
	CRTFeatures              []string `json:"crt_features"`
}

type Interpreter struct {
	Implementation         string
	PythonVersion          string
	GitHubRelease          string
	Triple                 string
	Config                 Config
	Flavor                 InterpreterFlavor
	Url                    string
	Info                   PythonJSON
	InterpreterImplemented *Interpreter
}

func (i Interpreter) GetKey() string {
	return fmt.Sprintf("%s-%s+%s-%s", i.Implementation, i.PythonVersion, i.GitHubRelease, i.Triple)
}

func GetBestInterpreter(interpreters []Interpreter) *Interpreter {
	// Sort by config. The first item will be the best match.
	sort.Sort(ByConfig(interpreters))

	return &interpreters[0]
}

func GetInterpreters(release GitHubRelease, threads int) ([]Interpreter, error) {
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
			key := interpreter.GetKey()
			groups[key] = append(groups[key], interpreter)
		}
	}

	// Cap the number of threads to not create more goroutines than necessary.
	if threads > len(installOnlyInterpreters) {
		threads = len(installOnlyInterpreters)
		fmt.Printf("Capping number of threads to %d\n", threads)
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

			interpreter.InterpreterImplemented = GetBestInterpreter(groups[interpreter.GetKey()])

			select {
			case inputChannel <- *interpreter.InterpreterImplemented:
				continue
			case <-ctx.Done():
				return ctx.Err()
			}
		}
		return nil
	})

	// Start a fixed number of goroutines to get the PYTHON.json files.
	outputChannel := make(chan Interpreter)
	for i := 0; i < threads; i++ {
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

// GetPythonInfo reads the python/PYTHON.json file inside an archive. The content
// is streamed and only the necessary bits are read.
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
