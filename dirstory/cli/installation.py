import os
import subprocess
from pathlib import Path

import click

from dirstory.constants import DirstoryBashrcText


def _get_dirstory_patch_location() -> str:
    click.echo("Trying to get location of dirstory's installation folder.")
    output = subprocess.check_output("pip show dirstory", shell=True)
    lines = output.decode().split("\n")
    location_line = list(filter(lambda line: "Location:" in line, lines))[0]
    actual_location = location_line.split(": ")[1]
    click.echo(f"Location found: {actual_location}")

    location_of_patch = actual_location + "/dirstory/scripts/_dirstorypatch"
    click.echo(f"Location of dirstory's bash script found: {location_of_patch}")
    return location_of_patch


def _is_dirstory_patch_already_installed() -> bool:
    bashrc_path = Path(os.path.expanduser("~/.bashrc"))
    if not bashrc_path.is_file():
        click.echo("~/.bashrc file not found")
        return False

    click.echo("~/.bashrc file found. Trying to find dirstory's config in it.")
    with open(bashrc_path, "r") as bashrc:
        return DirstoryBashrcText.START in bashrc.read()


def init_dirstory_patch() -> None:
    if _is_dirstory_patch_already_installed():
        click.echo("dirstory config already installed in ~/.bashrc.\n Skipping.")
        return

    click.echo("dirstory's config not found in ~/.bashrc... trying to make one.")
    with open(os.path.expanduser("~/.bashrc"), "a+") as bashrc:
        location = _get_dirstory_patch_location()
        click.echo("Writing a config script to ~/.bashrc file.")
        bashrc.write(DirstoryBashrcText.content_to_write(location))


@click.command("install")
def install() -> None:
    """Runs post install script needed to dirstory be fully functional."""
    click.echo("Running post install script")
    init_dirstory_patch()
    click.echo("Installation was successful.")
