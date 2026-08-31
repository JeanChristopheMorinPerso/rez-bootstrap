# rezup

`rezup` is a command-line tool intended to bootstrap and manage Rez installations.
The current implementation can list Rez versions available from GitHub. Other
executable actions return a nonzero `not implemented` error.

Build and inspect the CLI with Cargo:

```sh
cargo build
cargo run --bin rezup -- --help
cargo run --bin rezup -- list
cargo run --bin rezup -- list --json
```

The project is licensed under the Apache License 2.0.
