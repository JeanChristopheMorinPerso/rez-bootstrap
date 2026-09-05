use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use tempfile::Builder as TempDirBuilder;
use uv_python::downloads::PythonDownloadRequest;

use crate::{BuildMode, PythonPackageArgs};

const MAKE_PYTHON_PACKAGE: &[u8] = include_bytes!("../scripts/install_python_package.py");

pub fn install_python(rez: Option<PathBuf>, args: PythonPackageArgs) -> Result<()> {
    let request = download_request(&args)?;
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
    let python = runtime.block_on(crate::install::install_python_request(
        &request,
        &payload,
        staging.path(),
    ))?;

    install_rez_package(&rez, &payload, &python.version, args.release)?;
    eprintln!("Installed Python {} as a Rez package", python.version);
    Ok(())
}

fn download_request(args: &PythonPackageArgs) -> Result<PythonDownloadRequest> {
    if args.platform.is_some() || args.libc.is_some() {
        bail!("custom Python platform and libc selectors are not implemented yet");
    }
    let debug = matches!(args.mode, Some(BuildMode::Debug));
    let version = args
        .version
        .as_deref()
        .unwrap_or(if debug { "3" } else { "any" });
    if version.contains('+') {
        bail!("Python version variants must be selected with --mode");
    }
    let version = if debug {
        format!("{version}+debug")
    } else {
        version.to_owned()
    };
    let architecture = normalize_architecture(args.arch.as_deref(), args.microarch.as_deref())?;
    let selector = format!(
        "cpython-{version}-any-{}-any",
        architecture.as_deref().unwrap_or("any")
    );
    PythonDownloadRequest::from_str(&selector)
        .with_context(|| format!("invalid managed Python selectors `{selector}`"))?
        .fill()
        .context("failed to resolve managed Python selector defaults")
        .map(|request| request.with_prereleases(false))
}

fn normalize_architecture(arch: Option<&str>, microarch: Option<&str>) -> Result<Option<String>> {
    let arch = arch.map(|value| match value.to_ascii_lowercase().as_str() {
        "amd64" => "x86_64".to_owned(),
        "arm64" => "aarch64".to_owned(),
        value => value.to_owned(),
    });
    let Some(microarch) = microarch else {
        return Ok(arch);
    };
    let microarch = microarch.to_ascii_lowercase();
    if !matches!(microarch.as_str(), "v2" | "v3" | "v4") {
        bail!("unsupported Python microarchitecture `{microarch}`; expected v2, v3, or v4");
    }
    let arch = arch.as_deref().unwrap_or("x86_64");
    if arch != "x86_64" {
        bail!("Python microarchitecture `{microarch}` is only supported with x86_64");
    }
    Ok(Some(format!("{arch}_{microarch}")))
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

    #[test]
    fn creates_uv_request_from_python_build_selectors() {
        let args = PythonPackageArgs {
            version: Some("3.13".to_owned()),
            mode: Some(BuildMode::Debug),
            arch: Some("amd64".to_owned()),
            microarch: Some("v3".to_owned()),
            platform: None,
            libc: None,
            release: false,
        };

        let request = download_request(&args).unwrap().to_string();
        assert!(request.contains("3.13+debug"));
        assert!(request.contains("x86_64_v3"));
    }

    #[test]
    fn rejects_invalid_microarchitecture_combinations() {
        let args = PythonPackageArgs {
            version: None,
            mode: None,
            arch: Some("aarch64".to_owned()),
            microarch: Some("v3".to_owned()),
            platform: None,
            libc: None,
            release: false,
        };

        assert!(download_request(&args).is_err());
    }
}
