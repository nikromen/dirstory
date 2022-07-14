import click

from dirstory.stack import FileStack


@click.command("blist")
@click.argument("pid", type=int, nargs=1)
@click.option(
    "-s",
    "--size",
    required=True,
    type=int,
    help="The amount of paths in the stack to be returned.",
)
def blist(pid: int, size: int) -> None:
    """Returns first N directories in the backward history."""
    file_stack = FileStack(ppid=pid)
    result = file_stack.show_n_last_paths(size, is_forward=False)
    click.echo(result, nl=False)


@click.command("flist")
@click.argument("pid", type=int, nargs=1)
@click.option(
    "-s",
    "--size",
    required=True,
    type=int,
    help="The amount of paths in the stack to be returned.",
)
def flist(pid: int, size: int) -> None:
    """Returns first N directories in the forward history."""
    file_stack = FileStack(ppid=pid)
    result = file_stack.show_n_last_paths(size, is_forward=True)
    click.echo(result, nl=False)


@click.command("list")
@click.argument("pid", type=int, nargs=1)
@click.option(
    "-s",
    "--size-each-side",
    required=True,
    type=int,
    help="The amount of paths in the stacks to be returned in each side.",
)
def list_(pid: int, size: int) -> None:
    """
    Returns first N directories from each history stack.

    (e.g. 3 from backward and 3 from forward)
    """
    file_stack = FileStack(ppid=pid)
    result = file_stack.show_n_last_paths(
        size, is_forward=False
    ) + file_stack.show_n_last_paths(size, is_forward=True)
    click.echo(result, nl=False)
