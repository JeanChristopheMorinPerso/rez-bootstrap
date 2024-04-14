import typing


class SubParserMandatoryArgs(typing.TypedDict):
    #: help will be shown in the main CLI's help message
    help: str

    #: description will be shown in the subparser's help message.
    description: str


class SubParserArgs(SubParserMandatoryArgs, total=False):
    usage: str
    prog: str
