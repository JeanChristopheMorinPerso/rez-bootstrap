use assert_cmd::Command;
use predicates::prelude::*;

fn rezup() -> Command {
    Command::cargo_bin("rezup").expect("rezup binary should build")
}

#[test]
fn top_level_help_and_version_succeed() {
    rezup()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Bootstrap and manage Rez installations",
        ))
        .stdout(predicate::str::contains("package"))
        .stdout(predicate::str::contains("-v, --version"))
        .stdout(predicate::str::contains("-V, --version").not())
        .stderr(predicate::str::is_empty());

    for flag in ["-v", "--version"] {
        rezup()
            .arg(flag)
            .assert()
            .success()
            .stdout(predicate::str::contains("rezup 0.1.0"))
            .stderr(predicate::str::is_empty());
    }
}

#[test]
fn install_parses_selectors_and_returns_stub_error() {
    rezup()
        .args([
            "install",
            "--python-version",
            "3.13",
            "--python-platform",
            "linux",
            "--python-arch",
            "x86_64",
            "--python-microarch",
            "v3",
            "--python-mode",
            "release",
            "--python-libc",
            "glibc",
            "3.2.1",
            "/opt/rez",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rezup install is not implemented"));
}

#[test]
fn nested_package_commands_parse_and_name_the_leaf_action() {
    rezup()
        .args([
            "package",
            "--rez",
            "/usr/local/bin/rez",
            "install",
            "python",
            "3.12.4",
            "--mode",
            "debug",
            "--arch",
            "x86_64",
            "--microarch",
            "v2",
            "--platform",
            "linux",
            "--libc",
            "glibc",
            "--release",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "rezup package install python is not implemented",
        ));

    rezup()
        .args(["package", "install", "arch", "x86_64", "--release"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "rezup package install arch is not implemented",
        ));

    rezup()
        .args(["package", "list", "platform"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "rezup package list is not implemented",
        ));
}

#[test]
fn nested_help_succeeds_without_stub_error() {
    rezup()
        .args(["package", "install", "python", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--microarch"))
        .stdout(predicate::str::contains("--release"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn missing_and_invalid_arguments_are_rejected() {
    rezup()
        .args(["install", "3.2.1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("<PATH>"))
        .stderr(predicate::str::contains("not implemented").not());

    rezup()
        .args(["package", "list", "graphviz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'graphviz'"))
        .stderr(predicate::str::contains("not implemented").not());

    rezup()
        .args(["install", "--python-mode", "optimized", "3.2.1", "/tmp/rez"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'optimized'"))
        .stderr(predicate::str::contains("not implemented").not());
}

#[test]
fn remaining_leaf_actions_return_stub_errors() {
    for (args, action) in [
        (&["bootstrap"][..], "rezup bootstrap"),
        (&["update"][..], "rezup update"),
        (&["self", "update"][..], "rezup self update"),
    ] {
        rezup()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains(format!(
                "{action} is not implemented"
            )));
    }
}
