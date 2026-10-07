"""Exercise real rezup operations in a disposable repository (requires network access)."""

import configparser
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile


def run(*command):
    print("Running:", " ".join(map(str, command)), flush=True)
    result = subprocess.run(command, capture_output=True, text=True)
    print(result.stdout, end="", flush=True)
    print(result.stderr, end="", file=sys.stderr, flush=True)
    result.check_returncode()
    return result


def snapshot(root):
    """Compare entries, file contents, symlink targets, and POSIX permissions."""
    entries = {}
    for path in root.rglob("*"):
        mode = path.lstat().st_mode
        permissions = stat.S_IMODE(mode) if os.name != "nt" else None
        if path.is_symlink():
            value = ("symlink", os.readlink(path), permissions)
        elif path.is_dir():
            value = ("directory", permissions)
        else:
            value = ("file", hashlib.sha256(path.read_bytes()).hexdigest(), permissions)
        entries[path.relative_to(root)] = value
    return entries


def check_payload(variant, repository, platform, architecture, libc):
    root = Path(variant.root)
    assert root.resolve().is_relative_to(repository.resolve()), root
    metadata = json.loads((root / ".rezup/python-build-standalone.json").read_text())
    assert metadata["platform"] == platform, metadata
    assert metadata["architecture"] == architecture, metadata
    assert metadata["libc"] == libc, metadata
    assert metadata["mode"] == "release", metadata
    assert metadata["download_key"].startswith(f"cpython-{variant.version}-"), metadata
    requirements = [str(requirement) for requirement in variant.variant_requires]
    from rez.version import Requirement

    expected_ephemerals = [f".python.libc=={libc}", ".python.mode==release"]
    if architecture == "x86_64":
        expected_ephemerals.append(".python.x86_64_level-1+")
    assert metadata["microarchitecture"] is None, metadata
    assert metadata["ephemerals"] == expected_ephemerals, metadata
    for ephemeral in metadata["ephemerals"]:
        assert str(Requirement(ephemeral)) in requirements, (ephemeral, requirements)
    marker_path = root / (
        "Lib/EXTERNALLY-MANAGED"
        if platform == "windows"
        else "lib/python3.13/EXTERNALLY-MANAGED"
    )
    marker = configparser.ConfigParser()
    marker.read_string(marker_path.read_text())
    assert "managed by rezup" in marker["externally-managed"]["Error"]
    executable = root / ("python.exe" if platform == "windows" else "bin/python")
    assert executable.is_file(), executable
    return metadata


def runtime_probe():
    """Runs under the resolved payload interpreter, with no rez dependency."""
    assert sys.version_info[:2] == (3, 13), sys.version
    assert Path(sys.prefix).resolve() == Path(os.environ["REZ_PYTHON_ROOT"]).resolve()
    print("Resolved Python:", sys.executable)


def test_list(rezup):
    versions = json.loads(run(rezup, "list", "--json").stdout)
    assert isinstance(versions, list) and versions, versions
    assert all(isinstance(version, str) for version in versions), versions
    assert "3.4.0" in versions, versions


def test_install(rezup, prefix):
    run(rezup, "install", "--python-version", "3.13", "3.4.0", prefix)
    if os.name == "nt":
        python = prefix / "python.exe"
        rez = prefix / "Scripts/rez/rez.exe"
    else:
        python = prefix / "bin/python"
        rez = prefix / "bin/rez/rez"
    assert "3.4.0" in run(rez, "--version").stdout
    return python, rez


def python_variants(repository):
    from rez.packages import iter_packages
    from rez.system import system

    # Package creation happens in subprocesses; discard the previous lookup cache.
    system.clear_caches()
    packages = list(iter_packages("python", paths=[str(repository)]))
    assert len(packages) == 1, packages
    return list(packages[0].iter_variants())


def test_create_python(rezup, rez, repository):
    from rez.system import system

    create = [rezup, "package", "--rez", rez, "create", "python"]
    created = run(*create, "3.13")
    variants = python_variants(repository)
    assert len(variants) == 1, variants
    native = variants[0]
    native_requirements = [str(req) for req in native.variant_requires]
    platform = {"osx": "macos"}.get(system.platform, system.platform)
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(
        system.arch.lower(), system.arch.lower()
    )
    libc = "gnu" if platform == "linux" else "none"
    metadata = check_payload(native, repository, platform, architecture, libc)
    assert all(req in native_requirements for req in system.variant), native_requirements
    assert not created.stdout and f"({native.uri})" in created.stderr, created
    return native, metadata


def test_existing_python_is_skipped(rezup, rez, repository, native):
    before = snapshot(repository)
    existing = run(rezup, "package", "--rez", rez, "create", "python", str(native.version))
    assert not existing.stdout, existing.stdout
    assert f"variant is already installed ({native.uri})" in existing.stderr, existing.stderr
    assert "Preparing managed Python" not in existing.stderr, existing.stderr
    assert "Selected Python" not in existing.stderr, existing.stderr
    assert snapshot(repository) == before, "Repository changed on repeat creation"


def create_host_packages(repository):
    from rez.package_maker import make_package
    from rez.system import system

    # System-package creation is still stubbed; create test-only host dependencies.
    for requirement in system.variant:
        name, package_version = requirement.split("-", 1)
        with make_package(name, str(repository)) as package:
            package.version = package_version


def test_resolved_python_runs(rez, native, metadata):
    result = run(rez, "env", "python", "--", "python", "--version")
    assert result.stdout.strip() == f"Python {native.version}", result.stdout
    run(
        rez, "env", f"python=={native.version}", *metadata["ephemerals"],
        "--", "python", str(Path(__file__).resolve()), "--runtime-probe",
    )


def test_cross_target_variant(rezup, rez, repository, native, metadata):
    native_payload = snapshot(Path(native.root))
    native_requirements = [str(req) for req in native.variant_requires]

    # Use the same exact Python patch version, not a second independent selector.
    foreign_platform = "linux" if metadata["platform"] == "windows" else "windows"
    foreign_libc = "gnu" if foreign_platform == "linux" else "none"
    run(
        rezup, "package", "--rez", rez, "create", "python", str(native.version),
        "--platform", foreign_platform, "--arch", "x86_64", "--libc", foreign_libc,
    )
    variants = python_variants(repository)
    assert len(variants) == 2, variants
    native_again = next(variant for variant in variants if variant.uri == native.uri)
    assert [str(req) for req in native_again.variant_requires] == native_requirements
    foreign = next(variant for variant in variants if variant.uri != native.uri)
    check_payload(foreign, repository, foreign_platform, "x86_64", foreign_libc)
    expected_arch = "AMD64" if foreign_platform == "windows" else "x86_64"
    foreign_requirements = [str(req) for req in foreign.variant_requires]
    assert f"platform-{foreign_platform}" in foreign_requirements, foreign_requirements
    assert f"arch-{expected_arch}" in foreign_requirements, foreign_requirements
    assert snapshot(Path(native_again.root)) == native_payload, "Native payload changed"
    test_resolved_python_runs(rez, native_again, metadata)


def test_packages(rezup, rez, repository):
    # This part runs under the installed interpreter so rez's API is available.
    import rez as rez_module

    assert rez_module.__version__ == "3.4.0", rez_module.__version__
    assert sys.version_info[:2] == (3, 13), sys.version
    native, metadata = test_create_python(rezup, rez, repository)
    test_existing_python_is_skipped(rezup, rez, repository, native)
    create_host_packages(repository)
    test_resolved_python_runs(rez, native, metadata)
    test_cross_target_variant(rezup, rez, repository, native, metadata)


def main():
    if len(sys.argv) != 2:
        sys.exit("Usage: python scripts/test_e2e.py <rezup-binary>")
    rezup = Path(sys.argv[1]).resolve()
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    with tempfile.TemporaryDirectory(prefix="rezup-e2e-") as directory:
        work = Path(directory)
        os.environ.update(
            REZ_LOCAL_PACKAGES_PATH=str(work / "packages"),
            REZ_PACKAGES_PATH=str(work / "packages"),
            REZ_READ_PACKAGE_CACHE="false",
            REZ_WRITE_PACKAGE_CACHE="false",
        )
        test_list(rezup)
        python, rez = test_install(rezup, work / "prefix")
        run(
            python, Path(__file__).resolve(), "--test-packages",
            rezup, rez, work / "packages",
        )
    print("End-to-end tests passed.")


if __name__ == "__main__":
    if sys.argv[1:] == ["--runtime-probe"]:
        runtime_probe()
    elif sys.argv[1:2] == ["--test-packages"]:
        test_packages(*map(Path, sys.argv[2:]))
    else:
        main()
