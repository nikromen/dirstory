# DIRectory hiSTORY

#### Navigate through the history of visited directories in the command line.

Have you ever wanted a back and forward button in your terminal to navigate back and forth?
Dirstory allows it!

### Short description

Dirstory allows you to move in previously visited directories just like in file manager.
It's very similar to navigating back and forth between web pages in your web browser.

## Table of contents

<!-- toc -->

- [Installation](#installation)
- [Usage](#usage)

<!-- tocstop -->

## Installation

1. Install binary

2. Setup dirstory for your shell

   Initialize dirstory for your shell by adding the output of `dirstory init <SHELL>` in
   your shell's configuration file. To add a custom directory-change command, use
   `dirstory init <SHELL> --command <COMMAND>`.

   <details>
   <summary>sh</summary>
   > Add this line to your config (e.g, `~/.shrc`) file:
   >
   > ```sh
   > eval "$(dirstory init sh)"
   > ```
   </details>

   <details>
   <summary>bash</summary>
   > Add this line to your config (e.g, `~/.bashrc`) file:
   >
   > ```bash
   > eval "$(dirstory init bash)"
   > ```
   </details>

   <details>
   <summary>zsh</summary>
   > Add this line to your config (e.g, `~/.zshrc`) file:
   >
   > ```zsh
   > eval "$(dirstory init zsh)"
   > ```
   </details>

   <details>
   <summary>fish</summary>
   > Add this line to your config (e.g, `~/.config/fish/config.fish`) file:
   >
   > ```fish
   > dirstory init fish | source
   > ```
   </details>

## Usage

dirstory consist of two main commands

- b - goes back to previously visited directories
- f - goes forward to previously backed directories

If you want to go back by one step, just simply use:

```bash
$ b
```

and similarly to go forward by one step:

```bash
$ f
```

Open full interactive history with `dh`, `b -i`, or `f -i`.

For more options, use `b --help`, `f --help`, or `dirstory --help`.
See `man 1 dirstory` for commands and limitations, and `man 5 dirstory` for configuration.
