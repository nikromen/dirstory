import os
import subprocess
from pathlib import Path

import click

from dirstory.constants import DirstoryBashrcText


class PostInstallScript:
    def __init__(self) -> None:
        self._dirstory_location: str = ""

    @property
    def dirstory_location(self) -> str:
        if self._dirstory_location:
            return self._dirstory_location

        click.echo("Trying to get location of dirstory's installation folder.")
        output = subprocess.check_output("pip show dirstory", shell=True)
        lines = output.decode().split("\n")
        location_line = list(filter(lambda line: "Location:" in line, lines))[0]
        self._dirstory_location = location_line.split(": ")[1]
        click.echo(f"Location found: {self._dirstory_location}")
        return self._dirstory_location

    def _get_dirstory_script_location(self, script: str) -> str:
        location_of_patch = self.dirstory_location + f"/dirstory/scripts/{script}"
        if not Path(location_of_patch).is_file():
            raise FileNotFoundError(f"File: {location_of_patch} does not exist.")

        click.echo(
            f"Location of dirstory's bash script {script} found: {location_of_patch}"
        )
        return location_of_patch

    @staticmethod
    def _is_dirstory_already_installed() -> bool:
        bashrc_path = Path(os.path.expanduser("~/.bashrc"))
        if not bashrc_path.is_file():
            click.echo("~/.bashrc file not found")
            return False

        click.echo("~/.bashrc file found. Trying to find dirstory's config in it.")
        with open(bashrc_path, "r") as bashrc:
            return DirstoryBashrcText.START in bashrc.read()

    def init_dirstory_install(self) -> None:
        if self._is_dirstory_already_installed():
            click.echo("dirstory config already installed in ~/.bashrc.\n Skipping.")
            return

        click.echo("dirstory's config not found in ~/.bashrc... trying to make one.")
        with open(os.path.expanduser("~/.bashrc"), "a+") as bashrc:
            click.echo("Writing a config script to ~/.bashrc file.")
            bashrc.write(
                DirstoryBashrcText.content_to_write(
                    _dirstorypatch=self._get_dirstory_script_location("_dirstorypatch"),
                    b=self._get_dirstory_script_location("b"),
                    f=self._get_dirstory_script_location("f"),
                )
            )


@click.command("install")
def install() -> None:
    """Runs post install script needed to dirstory be fully functional."""
    click.echo("Running post install script")
    PostInstallScript().init_dirstory_install()
    click.echo("Installation was successful.")
