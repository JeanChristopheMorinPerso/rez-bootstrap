package main

import (
	"encoding/json"
	"fmt"
	"net/http"
)

// A GitHub release asset.
type GitHubReleaseAsset struct {
	Name               string `json:"name"`
	BrowserDownloadURL string `json:"browser_download_url"`
}

// A GitHub release.
type GitHubRelease struct {
	Assets []GitHubReleaseAsset `json:"assets"`
}

// Get the latest release fro GitHub.
func GetLatestRelease() (GitHubRelease, error) {
	var release GitHubRelease

	response, err := http.Get("https://api.github.com/repos/indygreg/python-build-standalone/releases/latest")

	if err != nil {
		return release, fmt.Errorf("failed to get latest release: %w", err)
	}

	defer response.Body.Close()

	if response.StatusCode != 200 {
		return release, fmt.Errorf("failed to get latest release: %s", response.Status)
	}

	err = json.NewDecoder(response.Body).Decode(&release)
	return release, err

}

// Parse parses the asset name and return an Interpreter.
func (asset GitHubReleaseAsset) Parse() (Interpreter, error) {
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
