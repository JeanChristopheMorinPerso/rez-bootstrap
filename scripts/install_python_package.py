"""Install a prepared Python runtime as a host-specific Rez package."""

import shutil
import sys

from rez.config import config
from rez.package_maker import make_package
from rez.packages import get_package_from_repository
from rez.system import system
from rez.version import Requirement


action, source, version, release = sys.argv[1:]
repository = config.release_packages_path if release == "true" else config.local_packages_path


def make_root(_variant, root):
    shutil.copytree(source, root, dirs_exist_ok=True)


if system.platform == "windows":
    commands = """\
env.PATH.prepend("{this.root}/Scripts")
env.PATH.prepend("{this.root}")
"""
else:
    commands = 'env.PATH.prepend("{this.root}/bin")'


variant = system.variant


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
