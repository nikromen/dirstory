import os
from pathlib import Path
from typing import Optional, List

from dirstory.constants import DIR_STACK_PATH_SUFFIX, DIR_STACKS_LOCATION, StackType


class FileStack:
    def __init__(self, ppid: int) -> None:
        self.forward_stack_path = DIR_STACKS_LOCATION / DIR_STACK_PATH_SUFFIX.format(
            type=StackType.forward, ppid=ppid
        )
        self.backward_stack_path = DIR_STACKS_LOCATION / DIR_STACK_PATH_SUFFIX.format(
            type=StackType.backward, ppid=ppid
        )

    def _get_stack_path(self, is_forward: bool) -> Path:
        return self.forward_stack_path if is_forward else self.backward_stack_path

    def pop(self, is_forward: bool) -> Optional[Path]:
        with open(self._get_stack_path(is_forward), "rb+") as stack_file:
            stack_file.seek(0, os.SEEK_END)
            cursor = stack_file.tell()

            # skip the last newline in the file
            cursor -= 2

            while cursor > 0 and stack_file.read(1) != b"\n":
                cursor -= 1
                stack_file.seek(cursor, os.SEEK_SET)

            if cursor < 0:
                return None

            if cursor != 0:
                cursor += 1

            stack_file.seek(cursor, os.SEEK_SET)
            last_line = stack_file.readline().decode()
            stack_file.truncate(cursor)
            return Path(last_line)

    def push(self, path: Path, is_forward: bool) -> Path:
        stack_path = self._get_stack_path(is_forward)
        parent_directory = stack_path.parents[0]
        parent_directory.mkdir(parents=True, exist_ok=True)

        with open(stack_path, "a+") as stack_file:
            stack_file.write(str(path) + "\n")

        return path

    def show_n_last_paths(self, n: int, is_forward: bool) -> List[str]:
        if n == 0:
            return []

        with open(self._get_stack_path(is_forward), "r") as stack_file:
            return stack_file.readlines()[-n:]

    def show_slice_of_paths(self, from_: int, to: int, is_forward: bool) -> List[str]:
        with open(self._get_stack_path(is_forward), "r") as stack_file:
            return stack_file.readlines()[from_:to]

    def erase_file_stack(self, is_forward: bool) -> None:
        stack_path = self._get_stack_path(is_forward)
        if not stack_path.is_file():
            return

        with open(stack_path, "w") as stack_file:
            stack_file.seek(os.SEEK_SET)
            stack_file.truncate()
