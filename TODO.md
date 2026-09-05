# rezup work tracker

## Completed

- [x] Create the Rust/Cargo project and `rezup` binary.
- [x] Define the complete command and argument hierarchy with clap.
- [x] Make all unimplemented leaf commands fail with clear nonzero errors.
- [x] Support standard help and `-v`/`--version` output.
- [x] Implement `rezup list` using published rez GitHub releases.
- [x] Support paginated, human-readable, and JSON version listings.
- [x] Add a shared HTTP client with a `rezup/<version>` user agent and request timeout.
- [x] Add focused CLI, pagination, draft-filtering, and output-format tests.
- [x] Implement initial `rezup install` support for latest or selected rez and managed Python on the current host.
- [x] Install latest or selected host-compatible Python Build Standalone runtimes as rez packages.
- [x] Support version, mode, architecture, microarchitecture, platform, libc, and repository selectors for Python rez packages.
- [x] Give Python mode, libc implementation, and x86-64 microarchitecture solver-visible identities using rez ephemerals.
- [x] Store Python Build Standalone selection metadata in each rez variant payload.
- [x] Mark managed Python installations and package payloads as externally managed according to PEP 668.

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
- [ ] Implement `rezup package [--rez <path>] create arch [version] [--release]`.
- [ ] Implement `rezup package [--rez <path>] create os [version] [--release]`.
- [ ] Implement `rezup package [--rez <path>] create platform [version] [--release]`.
- [ ] Implement `rezup package [--rez <path>] list [python|os|arch|platform]`.
- [ ] Implement `rezup self update`.

## Installation and packages

- [ ] Download and verify rez release archives.
- [ ] Download and verify managed Python artifacts.
- [ ] For every downloaded artifact, verify published SHA-256 checksums and available signatures or SLSA attestations, then store checksums, source details, verification results, SLSA provenance, SBOMs, and related provenance documents with the installation metadata.
- [ ] Make installation transactional and clean up partial failures.
- [ ] Define overwrite, upgrade, downgrade, and already-installed behavior.
- [ ] Implement rez executable discovery from `--rez` or `PATH`.
- [ ] Define package version resolution and debug/release selection behavior.
- [ ] Read the minimum glibc symbol version from Python Build Standalone's `PYTHON.json` and represent it as a solver-visible minimum requirement.
- [ ] Define how rez sites inject exact host mode, libc, and x86-64 microarchitecture capabilities so unconstrained resolves cannot select incompatible variants.
- [ ] Lock each rez package version around the final collision check and package publication to prevent concurrent writers from targeting the same variant root.
- [ ] Include `ManagedPythonDownload::build()` in the recorded artifact identity, and decide how a newer Python Build Standalone build of an existing Python variant should be represented and upgraded without overwriting an immutable rez package version.
- [ ] Figure out what to do woth hardcoded paths in ``sysconfig`. Patching `sysconfig` to the repository URI would defeat rez’s local payload cache.
- [ ] Relocate macOS Python dynamic-library install names and rpaths after moving the runtime into its final prefix or rez variant root.
- [ ] Verify that every generated Python executable, standard-library path, and linker/runtime path remains valid after the temporary staging directory is removed.

## Quality and delivery

- [ ] Manually review the CLI and write polished command, argument, and option descriptions.
- [ ] Manually write and review the README, usage guides, examples, and other user documentation.
- [ ] Add terminal progress bars with non-interactive CI behavior that hides them or uses stable line-oriented progress output.
- [ ] Add platform coverage for Windows, macOS, and Linux on supported architectures.
- [ ] Test filesystem changes, interrupted installs, and recovery paths.
- [ ] Test the standalone embedded package-creation script against a supported matrix of Python and rez versions independently from the `rezup` executable.
- [ ] Add end-to-end tests for multiple Python variants, solver selection, exact reinstall immutability, early download skipping, metadata sidecars, cross-target mapping, and package URI reporting.
- [ ] Define stable JSON schemas before consumers depend on them.
- [ ] Add release packaging and installation instructions for `rezup`.
- [ ] Ad-hoc code-sign macOS binaries with `codesign --sign -` and document the resulting Gatekeeper limitations; Developer ID signing and notarization are not planned.

## Out of scope

- Graphviz-specific commands or components.
