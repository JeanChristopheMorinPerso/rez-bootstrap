# rezup

`rezup` is a command-line tool intended to bootstrap and manage rez installations.
The current implementation can list rez versions available from GitHub and install
rez with a managed Python Build Standalone runtime. Other executable actions return
a nonzero `not implemented` error.

Build and inspect the CLI with Cargo:

```sh
cargo build
cargo run --bin rezup -- --help
cargo run --bin rezup -- list
cargo run --bin rezup -- list --json
cargo run --bin rezup -- install /opt/rez
cargo run --bin rezup -- install 3.4.0 /opt/rez
cargo run --bin rezup -- install --python-version 3.13 3.4.0 /opt/rez
cargo run --bin rezup -- package --rez /opt/rez/bin/rez/rez create python 3.13
```

Omitting the rez version selects the latest published release. Omitting
`--python-version` selects uv's latest stable CPython build for the current host.
Custom Python platform, architecture, microarchitecture, libc, and debug-build
selectors are not implemented yet.

`package create python` downloads a complete managed Python runtime and creates it
as a rez package. Its version, debug/release mode, architecture, x86-64
microarchitecture, platform, and libc selectors are matched against uv's Python Build
Standalone catalogue. Platform and base architecture are stored as rez variant
requirements. Mode and libc implementation use exact ephemeral requirements, while
x86-64 microarchitecture uses a minimum-level ephemeral requirement. Sites must provide
matching exact host capabilities when resolving these variants. Artifact selection
details are stored in `.rezup/python-build-standalone.json` inside each variant payload.
It uses the supplied `--rez` executable, or `rez`
from `PATH`, and installs into rez's configured local package repository by default;
`--release` selects the configured release repository. Existing matching variants are
detected from catalogue metadata and skipped before downloading, then checked again before
the package payload is copied. The completion message includes the selected variant URI whether
the variant is newly created or already exists. The installed interpreter includes a PEP 668
`EXTERNALLY-MANAGED` marker with a rezup-specific explanation. The embedded package-maker
is also available at `scripts/install_python_package.py` for standalone compatibility
testing with rez's Python.

The project is licensed under the Apache License 2.0.
