"""Install a prepared Python runtime as a host-specific Rez package."""

import shutil
import sys

from rez.config import config
from rez.package_maker import make_package
from rez.system import system


source, version, release = sys.argv[1:]
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


with make_package("python", repository, make_root=make_root) as package:
    package.version = version
    package.tools = ["python"]
    package.commands = commands
    package.variants = [system.variant]
