use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use tempfile::Builder as TempDirBuilder;
use uv_python::downloads::PythonDownloadRequest;

use crate::install::SelectedPython;
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

    let selected = runtime.block_on(crate::install::select_python(&request))?;
    if variant_exists(&rez, &selected, args.release)? {
        eprintln!("Python {} variant is already installed", selected.version);
        return Ok(());
    }

    eprintln!("Installing managed Python package payload...");
    let python = runtime.block_on(crate::install::install_selected_python(
        selected,
        &payload,
        staging.path(),
    ))?;

    install_rez_package(&rez, &payload, &python.selection, args.release)?;
    eprintln!(
        "Installed Python {} as a Rez package",
        python.selection.version
    );
    Ok(())
}

fn download_request(args: &PythonPackageArgs) -> Result<PythonDownloadRequest> {
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
    let platform = args.platform.as_deref().map(normalize_platform);
    let architecture = normalize_architecture(args.arch.as_deref(), args.microarch.as_deref())?;
    let libc = match (args.libc.as_deref(), platform.as_deref()) {
        (Some(libc), _) => normalize_libc(libc),
        (None, Some("linux")) => "gnu".to_owned(),
        (None, Some(_)) => "none".to_owned(),
        (None, None) => "any".to_owned(),
    };
    if platform.as_deref().is_some_and(|value| value != "linux") && libc != "none" {
        bail!("--libc can only select `none` when --platform is not `linux`");
    }
    let selector = format!(
        "cpython-{version}-{}-{}-{libc}",
        platform.as_deref().unwrap_or("any"),
        architecture.as_deref().unwrap_or("any")
    );
    PythonDownloadRequest::from_str(&selector)
        .with_context(|| format!("invalid managed Python selectors `{selector}`"))?
        .fill()
        .context("failed to resolve managed Python selector defaults")
        .map(|request| request.with_prereleases(false))
}

fn normalize_platform(platform: &str) -> String {
    match platform.to_ascii_lowercase().as_str() {
        "darwin" | "osx" => "macos".to_owned(),
        platform => platform.to_owned(),
    }
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

fn normalize_libc(libc: &str) -> String {
    match libc.to_ascii_lowercase().as_str() {
        "glibc" => "gnu".to_owned(),
        libc => libc.to_owned(),
    }
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

fn install_rez_package(
    rez: &Path,
    payload: &Path,
    python: &SelectedPython,
    release: bool,
) -> Result<()> {
    let output = run_package_script(rez, "install", payload, python, release)?;
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

fn variant_exists(rez: &Path, python: &SelectedPython, release: bool) -> Result<bool> {
    let output = run_package_script(rez, "check", Path::new(""), python, release)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(3) => Ok(false),
        _ => bail!(
            "Rez package preflight exited with {}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

fn run_package_script(
    rez: &Path,
    action: &str,
    payload: &Path,
    python: &SelectedPython,
    release: bool,
) -> Result<Output> {
    let mut child = Command::new(rez)
        .arg("python")
        .arg("-")
        .arg(action)
        .arg(payload)
        .arg(&python.version)
        .arg(if release { "true" } else { "false" })
        .arg(&python.download_key)
        .arg(&python.platform)
        .arg(&python.architecture)
        .arg(&python.libc)
        .arg(&python.mode)
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
        .with_context(|| format!("failed to wait for Rez package {action}"))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_package_uses_rez_repository_configuration() {
        let script = str::from_utf8(MAKE_PYTHON_PACKAGE).unwrap();
        assert!(script.contains("config.local_packages_path"));
        assert!(script.contains("config.release_packages_path"));
        assert!(script.contains("package.variants = [variant]"));
        assert!(script.contains(".python.libc=="));
        assert!(script.contains(".python.x86_64_level-"));
        assert!(script.contains(".python.mode=="));
        assert!(script.contains("variant_exists()"));
        assert!(!script.contains("def commands"));
    }

    #[test]
    fn creates_uv_request_from_python_build_selectors() {
        let args = PythonPackageArgs {
            version: Some("3.13".to_owned()),
            mode: Some(BuildMode::Debug),
            arch: Some("amd64".to_owned()),
            microarch: Some("v3".to_owned()),
            platform: Some("linux".to_owned()),
            libc: Some("glibc".to_owned()),
            release: false,
        };

        assert_eq!(
            download_request(&args).unwrap().to_string(),
            "cpython-3.13+debug-linux-x86_64_v3-gnu"
        );
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

    #[test]
    fn normalizes_macos_arm_and_non_linux_libc() {
        let args = PythonPackageArgs {
            version: Some("3.12.4".to_owned()),
            mode: Some(BuildMode::Release),
            arch: Some("arm64".to_owned()),
            microarch: None,
            platform: Some("osx".to_owned()),
            libc: None,
            release: true,
        };

        assert_eq!(
            download_request(&args).unwrap().to_string(),
            "cpython-3.12.4-macos-aarch64-none"
        );
    }
}
