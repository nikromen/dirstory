<ARGUMENTS>

# Use the shell's PID to identify its history session.
export DIRSTORY_SHELL_PID=$$
dirstory internal ensure "$PWD"

# Preserve the owning shell and last observed directory across reinitialization.
_dirstory_owner=${_dirstory_owner:-$$}
_dirstory_last_pwd=${_dirstory_last_pwd:-$PWD}

# Track changes only in the owning shell, not in its subshells.
_dirstory_parent() {
    [[ "$ZSH_SUBSHELL" = 0 && "$$" = "$_dirstory_owner" ]]
}

<COMMON>
<CUSTOM_COMMAND>

# Register once alongside existing directory-change hooks, without redefining chpwd.
typeset -ga chpwd_functions
if (( ! ${chpwd_functions[(Ie)_dirstory_observe]} )); then
    chpwd_functions+=(_dirstory_observe)
fi
