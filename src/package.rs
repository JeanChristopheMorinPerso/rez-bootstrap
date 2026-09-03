use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use tempfile::Builder as TempDirBuilder;

use crate::{BuildMode, PythonPackageArgs};

const MAKE_PYTHON_PACKAGE: &[u8] = include_bytes!("../scripts/install_python_package.py");

pub fn install_python(rez: Option<PathBuf>, args: PythonPackageArgs) -> Result<()> {
    validate_selectors(&args)?;
    let rez = rez.unwrap_or_else(|| PathBuf::from("rez"));
    validate_rez(&rez)?;
    crate::install::initialize_uv()?;

    let staging = TempDirBuilder::new()
        .prefix("rezup-package-python-")
        .tempdir()
        .context("failed to create Python package staging directory")?;
    let payload = staging.path().join("payload");
    let runtime = tokio::runtime::Runtime::new().context("failed to start async runtime")?;

    eprintln!("Installing managed Python package payload...");
    let python = runtime.block_on(crate::install::install_python(
        args.version.as_deref(),
        &payload,
        staging.path(),
    ))?;

    install_rez_package(&rez, &payload, &python.version, args.release)?;
    eprintln!("Installed Python {} as a Rez package", python.version);
    Ok(())
}

fn validate_selectors(args: &PythonPackageArgs) -> Result<()> {
    if args.arch.is_some()
        || args.microarch.is_some()
        || args.platform.is_some()
        || args.libc.is_some()
    {
        bail!(
            "custom Python platform, architecture, microarchitecture, and libc selectors are not implemented yet"
        );
    }
    if matches!(args.mode, Some(BuildMode::Debug)) {
        bail!("debug managed Python builds are not implemented yet");
    }
    Ok(())
}

fn validate_rez(rez: &Path) -> Result<()> {
    let output = Command::new(rez)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to run Rez executable `{}`", rez.display()))?;
    if !output.status.success() {
        bail!(
            "Rez executable `{}` failed its version check with {}\n{}{}",
            rez.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn install_rez_package(rez: &Path, payload: &Path, version: &str, release: bool) -> Result<()> {
    let mut child = Command::new(rez)
        .arg("python")
        .arg("-")
        .arg(payload)
        .arg(version)
        .arg(if release { "true" } else { "false" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run Rez executable `{}`", rez.display()))?;
    child
        .stdin
        .take()
        .context("failed to open Rez Python standard input")?
        .write_all(MAKE_PYTHON_PACKAGE)
        .context("failed to send embedded package installer to Rez Python")?;
    let output = child
        .wait_with_output()
        .context("failed to wait for Rez package installation")?;
    if !output.status.success() {
        bail!(
            "Rez package installation exited with {}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_package_uses_rez_repository_configuration() {
        let script = str::from_utf8(MAKE_PYTHON_PACKAGE).unwrap();
        assert!(script.contains("config.local_packages_path"));
        assert!(script.contains("config.release_packages_path"));
        assert!(script.contains("package.variants = [system.variant]"));
        assert!(!script.contains("def commands"));
    }
}
