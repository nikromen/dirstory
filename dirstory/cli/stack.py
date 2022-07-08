from pathlib import Path

import click

from dirstory.stack import FileStack


@click.command("push")
@click.argument("path")
@click.pass_obj
def cd_push(file_stack: FileStack, path: str) -> None:
    """
    Pushes to the backward directory stack a given PATH.

    This should ALWAYS remove the forward directory stack since this argument is called
    internally on each `cd` command thus there's nowhere to go forward to.
    """
    file_stack.erase_file_stack(is_forward=True)
    file_stack.push(path=Path(path), is_forward=False)
