% DIRSTORY(1) dirstory | General Commands Manual

# NAME

dirstory - navigate backward and forward through visited directories

# SYNOPSIS

**dirstory init** _SHELL_ \[**-c** _COMMAND_ | **--command** _COMMAND_\]

**dirstory** \[**-h** | **--help** | **-V** | **--version**\]

**b** \[_N_\]

**f** \[_N_\]

**b** (**-l** | **--list**) \[_N_\]

**f** (**-l** | **--list**) \[_N_\]

**b** (**-h** | **--help**)

**f** (**-h** | **--help**)

# DESCRIPTION

Dirstory tracks directory visits and provides the shell functions **b** and **f**
to navigate through them, like the back and forward buttons in a browser.
Load the shell integration before using these functions.

# INITIALIZATION

**dirstory init** prints initialization code for **bash**, **zsh**, **fish**, or **sh**.
Add the appropriate command to your shell configuration:

## Bash

```bash
eval "$(dirstory init bash)"
```

## Zsh

```zsh
eval "$(dirstory init zsh)"
```

## Fish

```fish
dirstory init fish | source
```

## sh

```sh
eval "$(dirstory init sh)"
```

## Custom command

The **-c** or **--command** option names the directory-change command; the default
is **cd**. For example:

```bash
eval "$(dirstory init bash --command cdir)"
```

In Bash and sh, use the configured command to record changes immediately.
Zsh and Fish also track ordinary **cd** when a custom command is configured.

Names must match the ASCII pattern `[A-Za-z_][A-Za-z0-9_]*`.
Shell keywords, names used by the integration, and names beginning with
`_dirstory_` are reserved.

# NAVIGATION

**b** _N_
: Move backward by up to _N_ visits; the default is one.

**f** _N_
: Move forward by up to _N_ visits; the default is one.

**-l** \[_N_\], **--list** \[_N_\]
: List neighboring visits in the chosen direction, nearest first.
The current visit is excluded. The default limit is ten; listing does not move
the cursor.

**-h**, **--help**
: Show help for the function without navigating or accessing history.

# HISTORY BEHAVIOR

A successful ordinary directory change discards the forward branch and adds a
new visit. Staying in the same directory adds nothing and preserves the forward
branch. Returning to a previously visited path later creates a separate visit.

Navigation with **b** and **f** moves the "cursor" without creating visits.
Failed directory changes leave history unchanged. If a selected directory is
missing or inaccessible, navigation fails and preserves the visit; it does not
skip it.

Paths are recorded as reported by the shell's **PWD**. Symlink aliases remain
distinct when the shell preserves them.

# LIMITATIONS

Bash records changes through the configured command immediately and checks for
other changes before the next interactive prompt or navigation command.
Several changes bypassing the configured command before that check may lose
intermediate visits. An unsupported **PROMPT_COMMAND** declaration disables
prompt tracking and produces a warning.

Zsh tracks directory changes through its directory-change hook. **cd -q**
explicitly suppresses hooks and is not tracked.

Fish observes changes to **PWD** and checks again before a prompt or navigation
command. Several changes together may lose intermediate visits. A Fish command
substitution that changes the main shell's directory is tracked as an actual
directory change.

sh records only changes through the configured directory-change command.

Paths containing a newline are unsupported.

# SEE ALSO

**dirstory**(5)
