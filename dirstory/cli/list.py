import click

from dirstory.stack import FileStack


@click.command("blist")
@click.option(
    "-s",
    "--size",
    required=True,
    type=int,
    help="The amount of paths in the stack to be returned.",
)
@click.pass_obj
def blist(file_stack: FileStack, size: int) -> None:
    """Returns first N directories in the backward history."""
    result = file_stack.show_n_last_paths(size, is_forward=False)
    click.echo(result, nl=False)


@click.command("flist")
@click.option(
    "-s",
    "--size",
    required=True,
    type=int,
    help="The amount of paths in the stack to be returned.",
)
@click.pass_obj
def flist(file_stack: FileStack, size: int) -> None:
    """Returns first N directories in the forward history."""
    result = file_stack.show_n_last_paths(size, is_forward=True)
    click.echo(result, nl=False)


@click.command("list")
@click.option(
    "-s",
    "--size-each-side",
    required=True,
    type=int,
    help="The amount of paths in the stacks to be returned in each side.",
)
@click.pass_obj
def list_(file_stack: FileStack, size: int) -> None:
    """
    Returns first N directories from each history stack.

    (e.g. 3 from backward and 3 from forward)
    """
    result = file_stack.show_n_last_paths(
        size, is_forward=False
    ) + file_stack.show_n_last_paths(size, is_forward=True)
    click.echo(result, nl=False)
