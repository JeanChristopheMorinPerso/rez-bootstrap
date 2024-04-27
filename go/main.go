package main

import (
	"fmt"
	"regexp"
)

type GitHubReleaseAsset struct {
	Name               string `json:"name"`
	BrowserDownloadURL string `json:"browser_download_url"`
}

// / A GitHub release.
type GitHubRelease struct {
	Assets []GitHubReleaseAsset `json:"assets"`
}

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
	release, err := GetLatestRelease()
	if err != nil {
		panic(err)
	}

	interpreters, err := GetInterpreters(release)
	if err != nil {
		panic(err)
	}

	for _, interpreter := range interpreters {
		fmt.Printf("%+v\n", interpreter.Info)
	}
}
