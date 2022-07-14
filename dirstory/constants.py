from enum import Enum
from pathlib import Path

VERSION = "0.1.0"

DIR_STACKS_LOCATION = Path("/tmp/dirstory")
DIR_STACK_PATH_SUFFIX = "{type}_dir_stack/{ppid}"

TILT = "~"
HOME_PATH = "/home/{username}"


class StackType(str, Enum):
    forward = "forward"
    backward = "backward"


class DirstoryBashrcText(str, Enum):
    START = (
        "# ==================== THIS SECTION WAS ADDED BY DIRSTORY "
        "====================\n"
    )
    VERSION = f"# dirstory version: {VERSION}\n"
    CONTENT = "source {location}\n"
    END = (
        "# ===================== END OF SECTION ADDED BY DIRSTORY "
        "=====================\n"
    )

    @classmethod
    def content_to_write(cls, location: str) -> str:
        return (
            cls.START
            + cls.VERSION
            + cls.CONTENT.value.format(location=location)
            + cls.END
        )
