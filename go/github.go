package main

import (
	"encoding/json"
	"fmt"
	"net/http"
)

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
