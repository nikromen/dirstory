from pathlib import Path
from unittest.mock import patch

import pytest
from click.testing import CliRunner

from dirstory.cli.stack import cd_push
from dirstory.stack import FileStack


class TestStack:
    @pytest.mark.parametrize(
        "path", [pytest.param("/some/path"), pytest.param("/different/path")]
    )
    @patch.object(FileStack, "push")
    @patch.object(FileStack, "erase_file_stack")
    def test_cd_push(self, mock_erase_file_stack, mock_push, path):
        # should only call method for erasing the forward stack
        # and push to backward stack
        file_stack = FileStack(ppid=123)
        runner = CliRunner()
        runner.invoke(cd_push, [path], obj=file_stack)

        mock_erase_file_stack.assert_called_once_with(is_forward=True)
        mock_push.assert_called_once_with(path=Path(path), is_forward=False)
