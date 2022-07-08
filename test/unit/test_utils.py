import os
from pathlib import Path

import pytest
from flexmock import flexmock

from dirstory.utils import abs2home


class TestUtils:
    @pytest.mark.parametrize(
        "input_path,login,result",
        [
            pytest.param(
                Path("/home/jozko"),
                "jozko",
                Path("~"),
            ),
            pytest.param(
                Path("/home/michael/something/other/here"),
                "michael",
                Path("~/something/other/here"),
            ),
            pytest.param(
                Path("/home/stupid-user/hello"),
                "stupid-user",
                Path("~/hello"),
            ),
            pytest.param(
                Path("/home/stupid-user/"),
                "stupid-user",
                Path("~/"),
            ),
            pytest.param(
                Path("/home/jozko"),
                "stupid-user",
                Path("/home/jozko"),
            ),
            pytest.param(
                Path("/usr/lib"),
                "stupid-user",
                Path("/usr/lib"),
            ),
        ],
    )
    def test_abs2home(self, input_path, login, result):
        flexmock(os).should_receive("getlogin").and_return(login)

        assert abs2home(input_path) == result
