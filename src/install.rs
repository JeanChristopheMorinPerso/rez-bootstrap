use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use serde::Deserialize;
use tar::Archive;
use tempfile::{Builder as TempDirBuilder, NamedTempFile};
use uv_cache::Cache;
use uv_client::BaseClientBuilder;
use uv_python::PythonRequest;
use uv_python::downloads::{
    DownloadResult, ManagedPythonDownload, ManagedPythonDownloadList, PythonDownloadRequest,
};
use uv_python::managed::ManagedPythonInstallation;

use crate::{BuildMode, InstallArgs};

const EXTERNALLY_MANAGED: &str = "[externally-managed]\n\
Error=This Python installation is managed by rezup and must not be modified directly. Create a virtual environment to install additional packages.\n";

pub(crate) struct ManagedPython {
    pub executable: PathBuf,
    pub selection: SelectedPython,
}

pub(crate) struct SelectedPython {
    download: ManagedPythonDownload,
    pub version: String,
    pub download_key: String,
    pub platform: String,
    pub architecture: String,
    pub libc: String,
    pub mode: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

pub fn run(args: InstallArgs) -> Result<()> {
    validate_selectors(&args)?;
    initialize_uv()?;

    let destination = std::path::absolute(&args.path)
        .with_context(|| format!("failed to resolve `{}`", args.path.display()))?;
    ensure_destination_available(&destination)?;
    let parent = destination
        .parent()
        .context("installation path must have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create `{}`", parent.display()))?;

    let runtime = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
    eprintln!("Installing managed Python...");
    let result = (|| {
        let python = runtime.block_on(install_python(
            args.python_version.as_deref(),
            &destination,
            parent,
        ))?;

        let rez_version = resolve_rez_version(&args.version)?;
        eprintln!("Installing Rez {rez_version}...");
        install_rez(&rez_version, &python.executable, &destination, parent)?;
        Ok(rez_version)
    })();
    let rez_version = match result {
        Ok(version) => version,
        Err(error) => {
            if let Err(cleanup_error) = fs::remove_dir_all(&destination)
                && cleanup_error.kind() != io::ErrorKind::NotFound
            {
                return Err(error).context(format!(
                    "additionally failed to remove partial installation `{}`: {cleanup_error}",
                    destination.display()
                ));
            }
            return Err(error);
        }
    };

    eprintln!("Installed Rez {rez_version} into {}", destination.display());
    Ok(())
}

pub(crate) fn initialize_uv() -> Result<()> {
    uv_preview::set(uv_preview::Preview::default())
        .context("failed to initialize uv preview configuration")?;
    uv_preview::finalize().context("failed to finalize uv preview configuration")?;
    Ok(())
}

fn validate_selectors(args: &InstallArgs) -> Result<()> {
    if args.python_platform.is_some()
        || args.python_arch.is_some()
        || args.python_microarch.is_some()
        || args.python_libc.is_some()
    {
        bail!(
            "custom Python platform, architecture, microarchitecture, and libc selectors are not implemented yet"
        );
    }
    if matches!(args.python_mode, Some(BuildMode::Debug)) {
        bail!("debug managed Python builds are not implemented yet");
    }
    Ok(())
}

fn ensure_destination_available(destination: &Path) -> Result<()> {
    match fs::symlink_metadata(destination) {
        Ok(_) => bail!(
            "installation path `{}` already exists",
            destination.display()
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect `{}`", destination.display()))
        }
    }
}

pub(crate) async fn install_python(
    version: Option<&str>,
    destination: &Path,
    parent: &Path,
) -> Result<ManagedPython> {
    let request = match version {
        Some(version) => PythonRequest::parse(&format!("cpython@{version}")),
        None => PythonRequest::parse("cpython"),
    };
    let request = PythonDownloadRequest::from_request(&request)
        .context("Python selector cannot be represented as a managed download")?
        .fill()
        .context("failed to detect the host Python platform")?
        .with_prereleases(false);
    install_python_request(&request, destination, parent).await
}

pub(crate) async fn install_python_request(
    request: &PythonDownloadRequest,
    destination: &Path,
    parent: &Path,
) -> Result<ManagedPython> {
    let selected = select_python(request).await?;
    install_selected_python(selected, destination, parent).await
}

pub(crate) async fn select_python(request: &PythonDownloadRequest) -> Result<SelectedPython> {
    let cache = Cache::temp().context("failed to create uv metadata cache")?;
    let client_builder = BaseClientBuilder::default()
        .custom_client(crate::http::async_client().context("failed to create HTTP client")?);
    let downloads = ManagedPythonDownloadList::new(&client_builder, &cache, None)
        .await
        .context("failed to load managed Python catalogue")?;
    let download = downloads
        .iter_matching(request)
        .next()
        .context("no matching stable managed Python build is available")?
        .clone();
    let key = download.key();
    let mode = if key.variant().is_debug() {
        "debug"
    } else {
        "release"
    }
    .to_owned();

    Ok(SelectedPython {
        version: key.version().to_string(),
        download_key: key.to_string(),
        platform: key.os().to_string(),
        architecture: key.arch().to_string(),
        libc: key.libc().to_string(),
        mode,
        download,
    })
}

pub(crate) async fn install_selected_python(
    selected: SelectedPython,
    destination: &Path,
    parent: &Path,
) -> Result<ManagedPython> {
    let client_builder = BaseClientBuilder::default()
        .custom_client(crate::http::async_client().context("failed to create HTTP client")?);
    let retry_policy = client_builder.retry_policy();
    let client = client_builder
        .clone()
        .retries(0)
        .build()
        .context("failed to create uv client")?;

    eprintln!("Selected Python {}", selected.download.key());
    let staging = TempDirBuilder::new()
        .prefix(".rezup-python-")
        .tempdir_in(parent)
        .with_context(|| {
            format!(
                "failed to create staging directory in `{}`",
                parent.display()
            )
        })?;
    let downloads_dir = staging.path().join("downloads");
    let scratch_dir = staging.path().join("scratch");
    tokio::fs::create_dir_all(&downloads_dir).await?;
    tokio::fs::create_dir_all(&scratch_dir).await?;

    let extracted = match selected
        .download
        .fetch_with_retry(
            &client,
            &retry_policy,
            &downloads_dir,
            &scratch_dir,
            false,
            None,
            None,
            None,
        )
        .await
        .context("failed to download managed Python")?
    {
        DownloadResult::AlreadyAvailable(path) | DownloadResult::Fetched(path) => path,
    };
    let payload = extracted.join("install");
    let payload = if payload.is_dir() { payload } else { extracted };
    fs::rename(&payload, destination).with_context(|| {
        format!(
            "failed to move managed Python into `{}`",
            destination.display()
        )
    })?;

    let installation =
        ManagedPythonInstallation::new(destination.to_path_buf(), &selected.download);
    installation.ensure_sysconfig_patched()?;
    installation.ensure_build_file()?;
    let key = installation.key();
    write_externally_managed(destination, key.os().is_windows(), key.major(), key.minor())?;
    let executable = ensure_canonical_executable(
        destination,
        key.os().is_windows(),
        key.major(),
        key.minor(),
        key.variant().executable_suffix(),
    )?;

    Ok(ManagedPython {
        executable,
        selection: selected,
    })
}

fn externally_managed_path(destination: &Path, windows: bool, major: u8, minor: u8) -> PathBuf {
    if windows {
        destination.join("Lib").join("EXTERNALLY-MANAGED")
    } else {
        destination
            .join("lib")
            .join(format!("python{major}.{minor}"))
            .join("EXTERNALLY-MANAGED")
    }
}

fn write_externally_managed(destination: &Path, windows: bool, major: u8, minor: u8) -> Result<()> {
    let path = externally_managed_path(destination, windows, major, minor);
    fs::write(&path, EXTERNALLY_MANAGED)
        .with_context(|| format!("failed to write PEP 668 marker `{}`", path.display()))?;
    Ok(())
}

fn ensure_canonical_executable(
    destination: &Path,
    windows: bool,
    major: u8,
    minor: u8,
    executable_suffix: &str,
) -> Result<PathBuf> {
    if windows {
        let executable = destination.join("python.exe");
        if !executable.is_file() {
            bail!(
                "managed Python archive does not contain `{}`",
                executable.display()
            );
        }
        return Ok(executable);
    }

    let executable = destination
        .join("bin")
        .join(format!("python{major}.{minor}{executable_suffix}"));
    if !executable.is_file() {
        bail!(
            "managed Python archive does not contain `{}`",
            executable.display()
        );
    }
    let canonical = executable.with_file_name("python");
    if !canonical.exists() {
        create_python_link(&executable, &canonical)?;
    }
    Ok(executable)
}

#[cfg(unix)]
fn create_python_link(executable: &Path, canonical: &Path) -> Result<()> {
    let target = executable
        .file_name()
        .context("managed Python executable has no file name")?;
    std::os::unix::fs::symlink(target, canonical).with_context(|| {
        format!(
            "failed to link `{}` to `{}`",
            canonical.display(),
            executable.display()
        )
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn create_python_link(executable: &Path, canonical: &Path) -> Result<()> {
    fs::copy(executable, canonical).with_context(|| {
        format!(
            "failed to copy `{}` to `{}`",
            executable.display(),
            canonical.display()
        )
    })?;
    Ok(())
}

fn resolve_rez_version(version: &str) -> Result<String> {
    if version != "latest" {
        if version.is_empty()
            || !version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            bail!("invalid Rez version `{version}`");
        }
        return Ok(version.to_owned());
    }

    crate::http::client()?
        .get("https://api.github.com/repos/AcademySoftwareFoundation/rez/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()?
        .error_for_status()?
        .json::<GitHubRelease>()
        .map(|release| release.tag_name)
        .context("failed to resolve the latest Rez version")
}

fn install_rez(version: &str, python: &Path, destination: &Path, parent: &Path) -> Result<()> {
    let url = format!(
        "https://github.com/AcademySoftwareFoundation/rez/archive/refs/tags/{version}.tar.gz"
    );
    let mut response = crate::http::client()?
        .get(&url)
        .send()?
        .error_for_status()
        .with_context(|| format!("failed to download Rez {version}"))?;
    let mut archive_file = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file in `{}`", parent.display()))?;
    io::copy(&mut response, &mut archive_file).context("failed to save Rez archive")?;

    let source = TempDirBuilder::new()
        .prefix(".rezup-rez-")
        .tempdir_in(parent)
        .with_context(|| {
            format!(
                "failed to create staging directory in `{}`",
                parent.display()
            )
        })?;
    Archive::new(GzDecoder::new(archive_file.reopen()?))
        .unpack(source.path())
        .context("failed to extract Rez archive")?;
    let source_root = fs::read_dir(source.path())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir() && path.join("install.py").is_file())
        .context("Rez archive does not contain install.py")?;

    // Rez's supported installer owns a virtual environment, so let it complete in staging and
    // merge its installed package into the managed Python prefix afterwards.
    let staged_rez = source.path().join("installed");
    let output = Command::new(python)
        .arg(source_root.join("install.py"))
        .arg(&staged_rez)
        .current_dir(&source_root)
        .output()
        .with_context(|| format!("failed to run `{}`", python.display()))?;
    if !output.status.success() {
        bail!(
            "Rez installer exited with {}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let staged_site_packages = find_directory(&staged_rez, "site-packages")
        .context("Rez installation does not contain site-packages")?;
    let destination_site_packages = python_purelib(python, destination)?;
    merge_directory(&staged_site_packages, &destination_site_packages)?;
    if let Some(dist_info) = fs::read_dir(&destination_site_packages)?
        .filter_map(Result::ok)
        .find(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            entry.path().is_dir() && name.starts_with("rez-") && name.ends_with(".dist-info")
        })
    {
        let metadata = serde_json::json!({
            "archive_info": {},
            "url": url,
        });
        fs::write(
            dist_info.path().join("direct_url.json"),
            serde_json::to_vec(&metadata)?,
        )?;
    }

    let output = Command::new(python)
        .arg("-c")
        .arg(
            "import install, sys; install.patch_rez_binaries(sys.argv[1]); install.copy_completion_scripts(sys.argv[1])",
        )
        .arg(destination)
        .current_dir(&source_root)
        .output()
        .context("failed to create Rez command wrappers")?;
    if !output.status.success() {
        bail!(
            "Rez wrapper installation exited with {}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let rez_commands = if cfg!(windows) {
        destination.join("Scripts").join("rez")
    } else {
        destination.join("bin").join("rez")
    };
    let rez_executable = rez_commands.join(if cfg!(windows) { "rez.exe" } else { "rez" });
    eprintln!("Verifying rez installation...");
    smoke_test_rez(&rez_executable)?;
    fs::write(rez_commands.join(".rez_production_install"), version)
        .context("failed to mark the Rez production installation")?;
    Ok(())
}

fn smoke_test_rez(rez: &Path) -> Result<()> {
    let output = Command::new(rez)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to run Rez smoke test `{}`", rez.display()))?;
    if !output.status.success() {
        bail!(
            "Rez smoke test `{}` exited with {}\n{}{}",
            rez.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn python_purelib(python: &Path, destination: &Path) -> Result<PathBuf> {
    let output = Command::new(python)
        .args([
            "-c",
            "import sysconfig; print(sysconfig.get_path('purelib'))",
        ])
        .output()
        .with_context(|| {
            format!(
                "failed to query `{}` for its purelib path",
                python.display()
            )
        })?;
    if !output.status.success() {
        bail!(
            "Python purelib query exited with {}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let path = std::str::from_utf8(&output.stdout)
        .context("Python purelib path is not valid UTF-8")?
        .trim();
    validate_install_path(path, destination)
}

fn validate_install_path(path: &str, destination: &Path) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty() || !path.is_absolute() || !path.starts_with(destination) {
        bail!(
            "Python reported invalid installation path `{}` outside `{}`",
            path.display(),
            destination.display()
        );
    }
    Ok(path)
}

fn find_directory(root: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|file_name| file_name == name) {
                return Some(path);
            }
            if let Some(found) = find_directory(&path, name) {
                return Some(found);
            }
        }
    }
    None
}

fn merge_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_directory(&source_path, &destination_path)?;
        } else if !destination_path.exists() {
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_rez_versions_without_network_access() {
        assert_eq!(resolve_rez_version("3.4.0").unwrap(), "3.4.0");
        assert!(resolve_rez_version("../3.4.0").is_err());
        assert!(resolve_rez_version("").is_err());
    }

    #[test]
    fn merges_directories_without_overwriting_existing_files() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("nested")).unwrap();
        fs::write(source.path().join("nested/new"), "new").unwrap();
        fs::write(source.path().join("existing"), "source").unwrap();
        fs::write(destination.path().join("existing"), "destination").unwrap();

        merge_directory(source.path(), destination.path()).unwrap();

        assert_eq!(
            fs::read_to_string(destination.path().join("nested/new")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(destination.path().join("existing")).unwrap(),
            "destination"
        );
    }

    #[test]
    fn accepts_only_python_install_paths_inside_the_prefix() {
        let prefix = tempfile::tempdir().unwrap();
        let site_packages = prefix.path().join("lib/python3.14/site-packages");
        assert_eq!(
            validate_install_path(site_packages.to_str().unwrap(), prefix.path()).unwrap(),
            site_packages
        );
        assert!(validate_install_path("lib/python3.14/site-packages", prefix.path()).is_err());
        let outside = prefix.path().with_extension("outside");
        assert!(validate_install_path(outside.to_str().unwrap(), prefix.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rez_smoke_test_requires_a_successful_command() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let rez = root.path().join("rez");
        fs::write(&rez, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&rez, fs::Permissions::from_mode(0o755)).unwrap();
        smoke_test_rez(&rez).unwrap();

        fs::write(&rez, "#!/bin/sh\necho broken >&2\nexit 9\n").unwrap();
        let error = smoke_test_rez(&rez).unwrap_err().to_string();
        assert!(error.contains("Rez smoke test"));
        assert!(error.contains("broken"));
    }

    #[test]
    fn locates_pep_668_markers_for_unix_and_windows_layouts() {
        let root = Path::new("python");
        assert_eq!(
            externally_managed_path(root, false, 3, 13),
            root.join("lib/python3.13/EXTERNALLY-MANAGED")
        );
        assert_eq!(
            externally_managed_path(root, true, 3, 13),
            root.join("Lib/EXTERNALLY-MANAGED")
        );
    }

    #[test]
    fn writes_rezup_pep_668_message() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("lib/python3.13")).unwrap();

        write_externally_managed(root.path(), false, 3, 13).unwrap();

        assert_eq!(
            fs::read_to_string(root.path().join("lib/python3.13/EXTERNALLY-MANAGED")).unwrap(),
            EXTERNALLY_MANAGED
        );
        assert!(EXTERNALLY_MANAGED.contains("managed by rezup"));
    }
}
