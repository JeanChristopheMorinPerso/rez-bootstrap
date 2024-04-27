package main

import (
	"fmt"
	"regexp"
	"runtime"

	"github.com/spf13/cobra"
)

var fullRegex = regexp.MustCompile(`^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triple>(?:-?[a-zA-Z0-9_])+)-(?P<config>debug|pgo\+lto|lto|noopt|pgo)-full.tar.zst$`)
var installOnlyRegex = regexp.MustCompile(`^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triple>(?:-?[a-zA-Z0-9_])+)-install_only\.tar\.gz$`)

func parseAsset(asset GitHubReleaseAsset) (Interpreter, error) {
	var interpreter Interpreter

	matches := fullRegex.FindAllStringSubmatch(asset.Name, -1)
	if len(matches) != 1 {
		matches = installOnlyRegex.FindAllStringSubmatch(asset.Name, -1)

		if len(matches) != 1 {
			return interpreter, fmt.Errorf("could not parse asset name: %s", asset.Name)
		}

		interpreter = Interpreter{
			Implementation: matches[0][installOnlyRegex.SubexpIndex("implementation")],
			PythonVersion:  matches[0][installOnlyRegex.SubexpIndex("pythonVersion")],
			GitHubRelease:  matches[0][installOnlyRegex.SubexpIndex("githubRelease")],
			Triple:         matches[0][installOnlyRegex.SubexpIndex("triple")],
			Flavor:         FlavorInstallOnly,
		}
	} else {
		config, err := ConfigFromString(matches[0][fullRegex.SubexpIndex("config")])
		if err != nil {
			return interpreter, err
		}

		interpreter = Interpreter{
			Implementation: matches[0][fullRegex.SubexpIndex("implementation")],
			PythonVersion:  matches[0][fullRegex.SubexpIndex("pythonVersion")],
			GitHubRelease:  matches[0][fullRegex.SubexpIndex("githubRelease")],
			Triple:         matches[0][fullRegex.SubexpIndex("triple")],
			Config:         config,
			Flavor:         FlavorFull,
		}
	}

	interpreter.Url = asset.BrowserDownloadURL
	return interpreter, nil
}

func main() {
	var threads int

	var rootCmd = &cobra.Command{
		Use:   "rez-bootstrap",
		Short: "A tool to bootstrap rez",
		Long:  "A tool to bootstrap rez",
		RunE: func(cmd *cobra.Command, args []string) error {
			release, err := GetLatestRelease()
			if err != nil {
				return err
			}

			interpreters, err := GetInterpreters(release, threads)
			if err != nil {
				return err
			}

			for _, interpreter := range interpreters {
				fmt.Printf("%s, %+v\n", interpreter.GetKey(), interpreter.Info)
			}
			return nil
		},
	}

	rootCmd.Flags().IntVarP(&threads, "threads", "t", runtime.NumCPU(), "Number of threads to use")

	rootCmd.Execute()
}
