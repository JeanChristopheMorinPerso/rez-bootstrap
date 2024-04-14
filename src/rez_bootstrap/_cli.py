import logging
import argparse
import importlib.metadata

import rez_bootstrap._python


def parseArgs() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "-v",
        "--version",
        action="version",
        version=importlib.metadata.version(__package__),
    )

    modules = [rez_bootstrap._python]

    for module in modules:
        subparsers = parser.add_subparsers()
        name, args = module.getParserArgs()
        subparser = subparsers.add_parser(name, **args)
        module.setupParser(subparser)
        subparser.set_defaults(func=module.run)

    return parser.parse_args()


def run():
    rootLogger = logging.getLogger(__package__)
    handler = logging.StreamHandler()
    handler.setFormatter(logging.Formatter("%(msg)s"))
    rootLogger.addHandler(handler)
    rootLogger.setLevel(logging.INFO)

    args = parseArgs()
    logging.getLogger(__name__).info("asd")
    if hasattr(args, "func"):
        args.func(args)
