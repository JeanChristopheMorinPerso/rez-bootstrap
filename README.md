# rezup

`rezup` is a command-line tool intended to bootstrap and manage Rez installations.
The current implementation can list Rez versions available from GitHub and install
Rez with a managed Python Build Standalone runtime. Other executable actions return
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
cargo run --bin rezup -- package --rez /opt/rez/bin/rez/rez install python 3.13
```

Omitting the Rez version selects the latest published release. Omitting
`--python-version` selects uv's latest stable CPython build for the current host.
Custom Python platform, architecture, microarchitecture, libc, and debug-build
selectors are not implemented yet.

`package install python` downloads a complete managed Python runtime and installs it
as a host-specific Rez package. It uses the supplied `--rez` executable, or `rez`
from `PATH`, and installs into Rez's configured local package repository by default;
`--release` selects the configured release repository. The embedded package-maker
is also available at `scripts/install_python_package.py` for standalone compatibility
testing with Rez's Python.

The project is licensed under the Apache License 2.0.
