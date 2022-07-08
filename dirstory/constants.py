from enum import Enum
from pathlib import Path


DIR_STACKS_LOCATION = Path("/tmp/dirstory")
DIR_STACK_PATH_SUFFIX = "{type}_dir_stack/{ppid}"

TILT = "~"
HOME_PATH = "/home/{username}"


class StackType(str, Enum):
    forward = "forward"
    backward = "backward"
