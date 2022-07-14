import click

from dirstory.cli.installation import install
from dirstory.cli.list import blist, flist, list_
from dirstory.cli.navigate import back, forward
from dirstory.cli.stack import cd_push


@click.group("dirstory")
def cli() -> None:
    """Navigate through directories in the terminal like in the file manager!"""
    pass


cli.add_command(blist)
cli.add_command(flist)
cli.add_command(list_)
cli.add_command(back)
cli.add_command(forward)
cli.add_command(cd_push)
cli.add_command(install)


if __name__ == "__main__":
    cli()
