from typing import Any

import click

from dirstory.cli.list import blist, flist, list_
from dirstory.cli.navigate import back, forward
from dirstory.cli.stack import cd_push
from dirstory.stack import FileStack


@click.group("dirstory")
@click.argument("pid", type=int, nargs=1)
@click.pass_context
def cli(ctx: Any, pid: int) -> None:
    """Navigate through directories in the terminal like in the file manager!"""
    ctx.obj = FileStack(ppid=pid)


cli.add_command(blist)
cli.add_command(flist)
cli.add_command(list_)
cli.add_command(back)
cli.add_command(forward)
cli.add_command(cd_push)


if __name__ == "__main__":
    cli()
