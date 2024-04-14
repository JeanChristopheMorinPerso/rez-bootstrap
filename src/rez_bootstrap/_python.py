import re
import json
import typing
import logging
import tarfile
import argparse
import collections
import dataclasses
import concurrent.futures

import rich
import requests
import zstandard
import rich.table

import rez_bootstrap._types


_LOG = logging.getLogger(__name__)

INSTALL_ONLY_REGEX = r"^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triplet>(?:-?[a-zA-Z0-9_])+)-install_only\.tar\.gz$"
FULL_REGEX = r"^(?P<implementation>\w+)-(?P<pythonVersion>.*)\+(?P<githubRelease>\d{8})-(?P<triplet>(?:-?[a-zA-Z0-9_])+)-(?P<config>debug|pgo\+lto|lto|noopt|pgo)-full.tar.zst$"


def getParserArgs() -> typing.Tuple[str, rez_bootstrap._types.SubParserArgs]:
    return "python", rez_bootstrap._types.SubParserArgs(
        help="Create a python package",
        description="The package will be created using a pre-compiled python",
    )


def setupParser(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "-l",
        "--list-available-versions",
        action="store_true",
        help="List available python versions",
    )


def run(args: argparse.Namespace) -> int:
    _LOG.info("Fetching list of assets from GH")
    interpreters = Interpreters()

    if args.list_available_versions:
        table = rich.table.Table(
            "Implementation", "Version", "Triplet", "Config", "C Runtime"
        )
        for variant in interpreters.variants:
            table.add_row(
                variant.implementation,
                variant.pythonVersion,
                variant.triplet,
                variant.config,
                " ".join(variant.cruntime),
            )
        rich.get_console().print(table)
    return 0


def getInfo(args: typing.Tuple["Variant", "Variant", "Variant"]):
    _LOG.info(f"Getting info for {args[0].url}")
    with requests.get(args[0].url, stream=True) as response:
        response.raise_for_status()

        dctx = zstandard.ZstdDecompressor()

        data = {}
        with dctx.stream_reader(response.raw) as reader:
            with tarfile.open(mode="r|", fileobj=reader) as tar:
                for member in tar:
                    if member.name != "python/PYTHON.json":
                        continue
                    data = json.load(tar.extractfile(member))
                    break
    return data, args[0], args[1]


@dataclasses.dataclass
class Variant:
    implementation: str
    pythonVersion: str
    githubRelease: str
    triplet: str
    config: typing.Literal["pgo+lto", "pgo", "lto", "noopt", "debug"] | None
    flavor: str
    url: str
    buildInfo: typing.Dict[str, typing.Any] = dataclasses.field(default_factory=dict)

    def __str__(self) -> str:
        return f"Variant(implementation={self.implementation!r}, pythonVersion={self.pythonVersion!r}, githubRelease={self.githubRelease!r}, triplet={self.triplet!r}, config={self.config!r}, flavor={self.flavor!r})"

    @property
    def cruntime(self) -> typing.List[str]:
        if "linux" in self.triplet:
            if "gnu" in self.triplet:
                maxSymbolVersion = [
                    feature
                    for feature in self.buildInfo["crt_features"]
                    if feature.startswith("glibc-max-symbol-version")
                ][0]
                return [f"glibc:{maxSymbolVersion.split(':', 1)[1]}"]
            return ["musl"]
        elif "darwin" in self.triplet:
            return [
                f"apple-sdk-deployment-target:{self.buildInfo['apple_sdk_deployment_target']}"
            ]
        else:
            return self.buildInfo["crt_features"]

    @property
    def arch(self) -> str:
        return self.triplet.split("-")[0]


class Interpreters:
    def __init__(self):
        with requests.get(
            "https://api.github.com/repos/indygreg/python-build-standalone/releases/latest",
            headers={"Accept": "application/json"},
        ) as response:
            response.raise_for_status()

            self._data = response.json()

        self.variants, self._groups = self._generateVariants()

    def _generateVariants(
        self,
    ) -> typing.Tuple[list[Variant], dict[typing.Tuple, typing.List[Variant]]]:
        variants: typing.List[Variant] = []
        groups = collections.defaultdict(list)

        for asset in sorted(self._data["assets"], key=lambda x: x["name"]):
            name = asset["name"]

            if not name.endswith((".tar.zst", "tar.gz")):
                continue

            if match := re.match(INSTALL_ONLY_REGEX, name):
                variant = Variant(
                    **match.groupdict(),
                    config=None,
                    flavor="install_only",
                    url=asset["browser_download_url"],
                )
                variants.append(variant)
            elif match2 := re.match(FULL_REGEX, name):
                print(match2.groupdict()["config"])
                variant = Variant(
                    **match2.groupdict(),
                    flavor="full",
                    url=asset["browser_download_url"],
                )
                groups[
                    (
                        match2.group("implementation"),
                        match2.group("pythonVersion"),
                        match2.group("triplet"),
                    )
                ].append(variant)
            else:
                print(f"{name!r} is not a reconigned python distribution")

        threadPool = concurrent.futures.ThreadPoolExecutor()
        itemsToProcess = []

        order = {"pgo+lto": 0, "pgo": 1, "lto": 2, "noopt": 3, None: 4, "debug": 5}
        for variant in variants:
            bestMatch: Variant = sorted(
                groups[
                    (variant.implementation, variant.pythonVersion, variant.triplet)
                ],
                key=lambda x: order.get(x.config, float("inf")),
            )[0]

            itemsToProcess.append([bestMatch, variant])

        for info, bestMatch, installOnly in threadPool.map(getInfo, itemsToProcess):
            installOnly.buildInfo = info
            installOnly.config = bestMatch.config

        return variants, groups
