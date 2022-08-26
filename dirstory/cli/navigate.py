import os
from pathlib import Path
from typing import Optional

import click

from dirstory.stack import FileStack


def back_or_forward(
    file_stack: FileStack, size: int, is_forward: bool
) -> Optional[Path]:
    curr_path = None
    for _ in range(size):
        curr_path = file_stack.pop(is_forward=is_forward)
        if curr_path is None:
            break

        file_stack.push(path=Path(os.getcwd()), is_forward=not is_forward)

    return curr_path


@click.command("back")
@click.argument("pid", type=int, nargs=1)
@click.option("-s", "--size", default=1, type=int, help="Goes back by N steps.")
def back(pid: int, size: int) -> None:
    """Returns a previously visited directories."""
    file_stack = FileStack(ppid=pid)
    click.echo(back_or_forward(file_stack, size, False), nl=False)


@click.command("forward")
@click.argument("pid", type=int, nargs=1)
@click.option("-s", "--size", default=1, type=int, help="Goes forward by N steps.")
def forward(pid: int, size: int) -> None:
    """Returns directories from which you last stepped back."""
    file_stack = FileStack(ppid=pid)
    click.echo(back_or_forward(file_stack, size, True), nl=False)
