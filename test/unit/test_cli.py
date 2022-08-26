from pathlib import Path
from unittest.mock import patch, call

import pytest
from click.testing import CliRunner

from dirstory.cli.stack import cd_push
from dirstory.cli.navigate import back, back_or_forward, forward
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
        runner = CliRunner()
        runner.invoke(cd_push, ["123", path])

        mock_erase_file_stack.assert_called_once_with(is_forward=True)
        mock_push.assert_called_once_with(path=Path(path), is_forward=False)


class TestNavigate:
    @pytest.mark.parametrize(
        "size,is_forward,expected",
        [
            pytest.param(2, True, Path("/path\n")),
            pytest.param(3, False, Path("/here\n")),
        ],
    )
    @patch("os.getcwd")
    @patch.object(FileStack, "pop")
    @patch.object(FileStack, "push")
    def test_back_or_forward(
        self, mock_push, mock_pop, mock_getcwd, size, is_forward, expected
    ):
        mocked_pop_ret_vals = [
            Path("/some\n"),
            Path("/path\n"),
            Path("/here\n"),
            Path("/dragon\n"),
        ]
        mock_pop.side_effect = mocked_pop_ret_vals
        mock_getcwd.side_effect = mocked_pop_ret_vals

        result = back_or_forward(FileStack(ppid=123), size, is_forward)

        assert mock_push.call_count == size
        assert mock_pop.call_count == size

        for i, mocked_pop_ret_val in zip(range(size), mocked_pop_ret_vals):
            assert mock_push.call_args_list[i] == call(
                path=mocked_pop_ret_val, is_forward=not is_forward
            )
            assert mock_pop.call_args_list[i] == call(is_forward=is_forward)

        assert expected == result

    @patch("dirstory.cli.navigate.back_or_forward")
    @patch("dirstory.cli.navigate.FileStack")
    def test_back_with_size(self, mock_file_stack, mock_back_or_forward):
        mock_file_stack.return_value = mock_file_stack

        runner = CliRunner()
        runner.invoke(back, ["--size", "3", "123"])

        mock_back_or_forward.assert_called_once_with(mock_file_stack, 3, False)

    @patch("dirstory.cli.navigate.back_or_forward")
    @patch("dirstory.cli.navigate.FileStack")
    def test_forward_with_size(self, mock_file_stack, mock_back_or_forward):
        mock_file_stack.return_value = mock_file_stack

        runner = CliRunner()
        runner.invoke(forward, ["--size", "3", "123"])

        mock_back_or_forward.assert_called_once_with(mock_file_stack, 3, True)

    @patch("dirstory.cli.navigate.back_or_forward")
    @patch("dirstory.cli.navigate.FileStack")
    def test_back_without_size(self, mock_file_stack, mock_back_or_forward):
        mock_file_stack.return_value = mock_file_stack

        runner = CliRunner()
        runner.invoke(back, ["123"])

        mock_back_or_forward.assert_called_once_with(mock_file_stack, 1, False)

    @patch("dirstory.cli.navigate.back_or_forward")
    @patch("dirstory.cli.navigate.FileStack")
    def test_forward_without_size(self, mock_file_stack, mock_back_or_forward):
        mock_file_stack.return_value = mock_file_stack

        runner = CliRunner()
        runner.invoke(forward, ["123"])

        mock_back_or_forward.assert_called_once_with(mock_file_stack, 1, True)
