import os

from pathlib import Path

from dirstory.constants import TILT, HOME_PATH


def abs2home(path: Path) -> Path:
    """
    Convert absolute path to relative to `~` if possible.

    :param path: Given absolute path
    :return: Whenever path contains home path prefix (e.g. /home/michael), replace
     this prefix with `~` and return this new path.
    """
    return Path(str(path).replace(HOME_PATH.format(username=os.getlogin()), TILT))
