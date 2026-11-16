% DIRSTORY(5) dirstory | File Formats Manual

# NAME

dirstory - directory history configuration

# DESCRIPTION

Dirstory supports an optional YAML or JSON configuration file. The only current
setting is **mode**, which determines which shell sessions share history.

# FILES

If **XDG_CONFIG_HOME** is set and nonempty, configuration is read from that
directory. Otherwise, it is read from **$HOME/.config**. If neither location is
available, the operation reports an error.

The first existing file in this order is used:

1. **dirstory.yaml**
2. **dirstory.yml**
3. **dirstory.json**

An invalid or unreadable selected file is an error; Dirstory does not fall back
to a lower-priority file. No configuration file, an empty or whitespace-only
file, or an empty object uses the defaults. Omitted settings also use defaults.

# OPTIONS

**mode: tty**
: The default. Identify history by the terminal associated with standard input.
Shells using the same terminal identifier share history, including a new shell
on that terminal. The terminal identifier must be available to commands accessing
history.

**mode: tmux**
: Identify history by the tmux pane from **TMUX_PANE**. Shells in the same pane
share history. A missing or invalid pane identifier is an error; no other mode
is substituted.

**mode: pid**
: Identify history by the shell process ID supplied by the shell integration via
**DIRSTORY_SHELL_PID**. Commands in the same initialized shell share history.
A new initialized shell uses another history. The variable must contain a
positive decimal process ID; a missing or invalid value is an error.

Session sharing does not enable recording directory changes in separate Bash,
Zsh, or Fish subprocesses.

# EXAMPLES

## YAML

```yaml
mode: tmux
```

## JSON

```json
{ "mode": "pid" }
```

# VALIDATION

Mode names are case-sensitive: **tty**, **tmux**, and **pid**. Invalid syntax,
invalid types, and unknown settings are errors.

# SEE ALSO

**dirstory**(1)
