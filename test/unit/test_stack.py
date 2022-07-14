import builtins
import os
from io import BytesIO, StringIO
from pathlib import Path
from unittest.mock import patch, mock_open

import pytest
from flexmock import flexmock

from dirstory.constants import StackType
from dirstory.stack import FileStack


class TestStack:
    @pytest.mark.parametrize(
        "ppid",
        [
            pytest.param(123),
            pytest.param(321),
        ],
    )
    def test_create_stack(self, ppid):
        stack = FileStack(ppid)
        assert stack.backward_stack_path == Path(
            f"/tmp/dirstory/{StackType.backward}_dir_stack/{ppid}"
        )
        assert stack.forward_stack_path == Path(
            f"/tmp/dirstory/{StackType.forward}_dir_stack/{ppid}"
        )

    @pytest.mark.parametrize(
        "is_forward,result",
        [
            pytest.param(
                True, Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123")
            ),
            pytest.param(
                False, Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123")
            ),
        ],
    )
    def test_get_stack_path(self, is_forward, result):
        stack = FileStack(123)
        assert stack._get_stack_path(is_forward) == result

    @pytest.mark.parametrize(
        "path,is_forward",
        [
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
            ),
        ],
    )
    def test_pop(self, path, is_forward):
        fake_file = BytesIO(b"foo\nbar\nbaz\n")
        fake_file.close = (
            lambda: None
        )  # do not close the file even if Jesus tells you to do so

        flexmock(builtins).should_receive("open").with_args(path, "rb+").and_return(
            fake_file
        )

        stack = FileStack(ppid=123)

        assert str(stack.pop(is_forward)) == "baz\n"
        fake_file.seek(os.SEEK_SET)
        assert fake_file.read() == b"foo\nbar\n"

        assert str(stack.pop(is_forward)) == "bar\n"
        fake_file.seek(os.SEEK_SET)
        assert fake_file.read() == b"foo\n"

        assert str(stack.pop(is_forward)) == "foo\n"
        fake_file.seek(os.SEEK_SET)
        assert fake_file.read() == b""

        assert stack.pop(is_forward) is None
        fake_file.seek(os.SEEK_SET)
        assert fake_file.read() == b""

        assert stack.pop(is_forward) is None
        fake_file.seek(os.SEEK_SET)
        assert fake_file.read() == b""

    @pytest.mark.parametrize(
        "path,is_forward",
        [
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
            ),
        ],
    )
    def test_push(self, path, is_forward):
        fake_open = mock_open()
        stack = FileStack(ppid=123)
        push_path = Path("/some/path")
        with patch("builtins.open", fake_open, create=True):
            ret_val = stack.push(path=push_path, is_forward=is_forward)

        fake_open.assert_called_with(path, "a+")
        fake_open.return_value.write.assert_called_once_with("/some/path\n")
        assert ret_val == push_path

    @pytest.mark.parametrize(
        "path,is_forward,n,content,result",
        [
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
                2,
                "foo\nbar\nbaz\n",
                ["bar\n", "baz\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
                6,
                "foo\nbar\nbaz\nomg\nsome\nlines\n",
                ["foo\n", "bar\n", "baz\n", "omg\n", "some\n", "lines\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
                3,
                "foo\nbar\nbaz\nomg\nsome\nlines\n",
                ["omg\n", "some\n", "lines\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
                6,
                "foo\nbar\nbaz\n",
                ["foo\n", "bar\n", "baz\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
                0,
                "foo\nbar\nbaz\n",
                [],
            ),
        ],
    )
    def test_show_n_last_paths(self, path, is_forward, n, content, result):
        fake_file = StringIO(content)
        flexmock(builtins).should_receive("open").with_args(path, "r").and_return(
            fake_file
        )

        stack = FileStack(ppid=123)
        assert stack.show_n_last_paths(n, is_forward) == result

    @pytest.mark.parametrize(
        "path,is_forward,from_,to,content,result",
        [
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
                0,
                1,
                "foo\nbar\nbaz\n",
                ["foo\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
                2,
                5,
                "foo\nbar\nbaz\nomg\nsome\nlines\n",
                ["baz\n", "omg\n", "some\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
                0,
                1,
                "foo\nbar\nbaz\nomg\nsome\nlines\n",
                ["foo\n"],
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
                1,
                89,
                "foo\nbar\nbaz\n",
                ["bar\n", "baz\n"],
            ),
        ],
    )
    def test_show_slice_of_paths(self, path, is_forward, from_, to, content, result):
        fake_file = StringIO(content)
        flexmock(builtins).should_receive("open").with_args(path, "r").and_return(
            fake_file
        )

        stack = FileStack(ppid=123)
        assert stack.show_slice_of_paths(from_, to, is_forward) == result

    @pytest.mark.parametrize(
        "path,is_forward",
        [
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.forward}_dir_stack/123"),
                True,
            ),
            pytest.param(
                Path(f"/tmp/dirstory/{StackType.backward}_dir_stack/123"),
                False,
            ),
        ],
    )
    @patch.object(Path, "is_file")
    def test_erase_stack(self, mock_pathlib_is_file, path, is_forward):
        fake_file = StringIO("/some/path\n/other/path\n/different/path\n")
        fake_file.close = (
            lambda: None
        )  # do not close the file even if Jesus tells you to do so

        flexmock(builtins).should_receive("open").with_args(path, "w").and_return(
            fake_file
        )

        mock_pathlib_is_file.return_value = True

        stack = FileStack(ppid=123)
        stack.erase_file_stack(is_forward)
        fake_file.seek(os.SEEK_SET)
        assert fake_file.read() == ""
