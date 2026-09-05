# rezup work tracker

## Completed

- [x] Create the Rust/Cargo project and `rezup` binary.
- [x] Define the complete command and argument hierarchy with clap.
- [x] Make all unimplemented leaf commands fail with clear nonzero errors.
- [x] Support standard help and `-v`/`--version` output.
- [x] Implement `rezup list` using published Rez GitHub releases.
- [x] Support paginated, human-readable, and JSON version listings.
- [x] Add a shared HTTP client with a `rezup/<version>` user agent and request timeout.
- [x] Add focused CLI, pagination, draft-filtering, and output-format tests.
- [x] Implement initial `rezup install` support for latest or selected Rez and managed Python on the current host.
- [x] Install latest or selected host-compatible Python Build Standalone runtimes as Rez packages.
- [x] Support version, mode, architecture, and microarchitecture selectors for Python Rez packages.

## HTTP reliability

- [ ] Retry transient connection failures, resets, and timeouts.
- [ ] Retry transient HTTP responses such as 408, 429, 500, 502, 503, and 504.
- [ ] Use bounded exponential backoff with jitter and a maximum attempt count.
- [ ] Honor `Retry-After` and applicable GitHub rate-limit reset headers.
- [ ] Do not retry deterministic client errors or malformed responses.
- [ ] Improve diagnostics for GitHub API errors without exposing credentials.
- [ ] Decide whether to support an optional GitHub token for higher rate limits.
- [ ] Decide on caching, ETag revalidation, and offline behavior for release listings.
- [ ] Add deterministic tests for retries, backoff limits, rate limiting, and terminal errors.

## Commands

- [ ] Add suitable short aliases for long options, starting with `-j`/`--json`.
- [ ] Add pager output similar to `less` for long human-readable output from list commands, while keeping JSON and redirected output unpaged.
- [ ] Implement `rezup bootstrap`.
- [ ] Implement the remaining `rezup install` Python platform, architecture, microarchitecture, libc, and debug-build selectors.
- [ ] Implement `rezup update`.
- [ ] Implement `rezup package [--rez <path>] install arch [version] [--release]`.
- [ ] Implement `rezup package [--rez <path>] install os [version] [--release]`.
- [ ] Implement `rezup package [--rez <path>] install platform [version] [--release]`.
- [ ] Implement the remaining `rezup package install python` platform and libc selectors.
- [ ] Implement `rezup package [--rez <path>] list [python|os|arch|platform]`.
- [ ] Implement `rezup self update`.

## Installation and packages

- [ ] Download and verify Rez release archives.
- [ ] Download and verify managed Python artifacts.
- [ ] For every downloaded artifact, verify published SHA-256 checksums and available signatures or SLSA attestations, then store checksums, source details, verification results, SLSA provenance, SBOMs, and related provenance documents with the installation metadata.
- [ ] Make installation transactional and clean up partial failures.
- [ ] Define overwrite, upgrade, downgrade, and already-installed behavior.
- [ ] Implement Rez executable discovery from `--rez` or `PATH`.
- [ ] Define package version resolution and debug/release selection behavior.

## Quality and delivery

- [ ] Manually review the CLI and write polished command, argument, and option descriptions.
- [ ] Manually write and review the README, usage guides, examples, and other user documentation.
- [ ] Add terminal progress bars with non-interactive CI behavior that hides them or uses stable line-oriented progress output.
- [ ] Add platform coverage for Windows, macOS, and Linux on supported architectures.
- [ ] Test filesystem changes, interrupted installs, and recovery paths.
- [ ] Define stable JSON schemas before consumers depend on them.
- [ ] Add release packaging and installation instructions for `rezup`.
- [ ] Ad-hoc code-sign macOS binaries with `codesign --sign -` and document the resulting Gatekeeper limitations; Developer ID signing and notarization are not planned.

## Out of scope

- Graphviz-specific commands or components.
