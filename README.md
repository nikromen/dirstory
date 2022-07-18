# DIRectory hiSTORY

#### Navigate through the directories via command line as in file manager!

Have you ever wanted a back and forward button in your terminal to navigate back and forth?
Dirstory allows it!

### Short description

Dirstory allows you to move in previously visited directories just like in file manager.
It's very similar to navigating back and forth between web pages in your web browser.

## Table of contents

<!-- toc -->

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [Contribution guidelines](#contribution-guidelines)

<!-- tocstop -->

## Requirements

- bash
- pip

## Installation

To install dirstory, you need to firstly install the cli and then run the post-install script via
command `dirstory install` to make the dirstory work properly.

### Installation of the cli

From PyPi:

```bash
$ pip install --user dirstory
```

Or you can install dirstory from the git repository:

```bash
$ pip install --user git+https://github.com/nikromen/dirstory
```

### Running the post-install script

After successful installation of the cli, run this command:

```bash
$ dirstory install
```

#### Note: what dirstory's post install script does

Please note that dirstory needs to add additional functionality to the built-in shell command
`cd` to trace in which directories have you been to redirect you there later.

If you already added some functionality to the built-in `cd` command, it's very likely that dirstory
or your script won't work. In that case, please check out the
[redefinition of the `cd` command that dirstory does](https://github.com/nikromen/dirstory/blob/main/dirstory/scripts/_dirstorypatch)
and you will need to merge this script with yours.

##### How to find location of [\_dirstorypatch](https://github.com/nikromen/dirstory/blob/main/dirstory/scripts/_dirstorypatch)

See the location of this script in your `~/.bashrc` file. There should be a block added by
dirstory with line: `source /some/path/to/the/_dirstorypatch`. You can edit this to make your
and dirstory's script work properly.

## Usage

dirstory consist of two main commands

- [b](#b)
- [f](#f)

### b

```
Goes to a previously visited directories.

Options:
    INTEGER     Goes back by N steps.
    -l INTEGER  Show last N directiories where you were.
    --help      Show this message and exit.
```

### f

```
Goes to a previously backed directories.

Options:
    INTEGER     Goes forward by N steps.
    -l INTEGER  Show last N directiories which you backed.
    --help      Show this message and exit.
```

---

If you want to go back by one step, just simply use:

```bash
$ b
```

and similarly to go forward by one step:

```bash
$ f
```

## Contribution guideline

TODO
