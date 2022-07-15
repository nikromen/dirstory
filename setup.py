#!/usr/bin/python3

from setuptools import setup

from dirstory.constants import VERSION

if __name__ == "__main__":
    setup(
        version=VERSION,
        package_data={"": ["scripts/_dirstorypatch", "scripts/b", "scripts/f"]},
        include_package_data=True,
    )
