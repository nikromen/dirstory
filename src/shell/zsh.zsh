# Tracking state survives repeated initialization.
dirstory internal ensure "$PWD"
_dirstory_owner=${_dirstory_owner:-$$}
_dirstory_last_pwd=${_dirstory_last_pwd:-$PWD}

_dirstory_parent() {
    [[ "$ZSH_SUBSHELL" = 0 && "$$" = "$_dirstory_owner" ]]
}

<COMMON>
<CUSTOM_COMMAND>

# Register alongside user hooks, without redefining chpwd.
typeset -ga chpwd_functions
if (( ! ${chpwd_functions[(Ie)_dirstory_observe]} )); then
    chpwd_functions+=(_dirstory_observe)
fi
