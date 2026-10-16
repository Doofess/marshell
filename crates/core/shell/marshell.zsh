# Marshell shell integration for zsh. The startup files in zsh/ load your own ones first and
# then this file. Nothing of yours is edited.
# It marks each prompt and command (OSC 133) and reports the folder (OSC 7), so Marshell can
# show whether a command is running and how the last one ended.

if [[ -n ${MARSHELL_SHELL_INTEGRATION-} ]]; then
    return 0
fi
MARSHELL_SHELL_INTEGRATION=1

typeset -g __marshell_ran= __marshell_status=0

# First in precmd: save the status before any other hook changes it.
__marshell_save_status() {
    __marshell_status=$?
}

# Last in precmd: report how the last command ended, the folder, and mark the prompt start.
__marshell_precmd() {
    if [[ -n $__marshell_ran ]]; then
        printf '\e]133;D;%s\e\\' "$__marshell_status"
    else
        printf '\e]133;D\e\\'
    fi
    __marshell_ran=
    printf '\e]7;file://%s%s\e\\' "${HOST-}" "$PWD"
    if [[ $PS1 != *'133;A'* ]]; then
        PS1=$'%{\e]133;A\e\\%}'"$PS1"
    fi
}

__marshell_preexec() {
    __marshell_ran=1
    printf '\e]133;C\e\\'
}

precmd_functions=(__marshell_save_status $precmd_functions __marshell_precmd)
preexec_functions+=(__marshell_preexec)
