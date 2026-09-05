"""Install a prepared Python runtime as a Rez package variant."""

import json
import shutil
import sys
from pathlib import Path

from rez.config import config
from rez.package_maker import make_package
from rez.packages import get_package_from_repository
from rez.system import system
from rez.version import Requirement


action, source, version, release, download_key, pbs_platform, pbs_arch, pbs_libc, mode = sys.argv[1:]
repository = config.release_packages_path if release == "true" else config.local_packages_path


def make_root(_variant, root):
    shutil.copytree(source, root, dirs_exist_ok=True)
    metadata_dir = Path(root) / ".rezup"
    metadata_dir.mkdir()
    (metadata_dir / "python-build-standalone.json").write_text(
        json.dumps(
            {
                "architecture": pbs_arch,
                "download_key": download_key,
                "ephemerals": ephemerals,
                "libc": pbs_libc,
                "libc_version": None,
                "microarchitecture": microarchitecture,
                "mode": mode,
                "platform": pbs_platform,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


commands = """\
env.PATH.prepend("{this.root}")
env.PATH.prepend("{this.root}/Scripts")
env.PATH.prepend("{this.root}/bin")
"""


arch_parts = pbs_arch.rsplit("_", 1)
if len(arch_parts) == 2 and arch_parts[1] in ("v2", "v3", "v4"):
    base_arch, microarchitecture = arch_parts
else:
    base_arch = pbs_arch
    microarchitecture = None

rez_platform = {"macos": "osx"}.get(pbs_platform, pbs_platform)
if rez_platform == "windows":
    rez_arch = {"x86_64": "AMD64", "aarch64": "ARM64"}.get(base_arch, base_arch)
elif rez_platform == "osx" and base_arch == "aarch64":
    rez_arch = "arm64"
else:
    rez_arch = base_arch


def comparable_arch(arch):
    return {"amd64": "x86_64", "arm64": "aarch64"}.get(arch.lower(), arch.lower())


if rez_platform == system.platform and comparable_arch(rez_arch) == comparable_arch(system.arch):
    variant = list(system.variant)
else:
    variant = [f"platform-{rez_platform}", f"arch-{rez_arch}"]

ephemerals = [f".python.libc=={pbs_libc}", f".python.mode=={mode}"]
if base_arch == "x86_64":
    microarchitecture_level = {None: 1, "v2": 2, "v3": 3, "v4": 4}[microarchitecture]
    ephemerals.append(f".python.x86_64_level-{microarchitecture_level}+")
variant.extend(ephemerals)


def variant_exists():
    package = get_package_from_repository("python", version, repository)
    if package is None:
        return False
    requested = tuple(str(Requirement(requirement)) for requirement in variant)
    return any(
        tuple(str(requirement) for requirement in existing.variant_requires) == requested
        for existing in package.iter_variants()
    )


if action == "check":
    sys.exit(0 if variant_exists() else 3)
if action != "install":
    raise RuntimeError(f"Unknown action: {action}")


with make_package("python", repository, make_root=make_root) as package:
    package.version = version
    package.tools = ["python"]
    package.commands = commands
    package.variants = [variant]
