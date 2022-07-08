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

        file_stack.push(path=curr_path, is_forward=not is_forward)

    return curr_path


@click.command("back")
@click.option("-s", "--size", default=1, type=int, help="Goes back by N steps.")
@click.pass_obj
def back(file_stack: FileStack, size: int) -> None:
    """Returns a previously visited directories."""
    click.echo(back_or_forward(file_stack, size, False), nl=False)


@click.command("forward")
@click.option("-s", "--size", default=1, type=int, help="Goes forward by N steps.")
@click.pass_obj
def forward(file_stack: FileStack, size: int) -> None:
    """Returns directories from which you last stepped back."""
    click.echo(back_or_forward(file_stack, size, True), nl=False)
