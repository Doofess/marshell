# Marshell shell integration for bash (3.2 and later). Marshell starts bash with this file as
# --init-file. It loads your own startup files first, the way bash would, and never edits them.
# It marks each prompt and command (OSC 133) and reports the folder (OSC 7), so Marshell can
# show whether a command is running and how the last one ended.

if [ -n "${MARSHELL_SHELL_INTEGRATION:-}" ]; then
    return 0 2>/dev/null
fi
MARSHELL_SHELL_INTEGRATION=1

if [ "${MARSHELL_BASH_LOGIN:-}" = 1 ]; then
    unset MARSHELL_BASH_LOGIN
    [ -r /etc/profile ] && . /etc/profile
    if [ -r ~/.bash_profile ]; then
        . ~/.bash_profile
    elif [ -r ~/.bash_login ]; then
        . ~/.bash_login
    elif [ -r ~/.profile ]; then
        . ~/.profile
    fi
else
    [ -r ~/.bashrc ] && . ~/.bashrc
fi

__marshell_ran=
__marshell_in_prompt=
# bash 4.4 and later with promptvars on: PS0 marks a command's start. Bash expands PS0 once per executed command
# line and not for an empty, comment-only or syntax-error line. The subscript's arithmetic sets __marshell_ran in
# the shell, the element is unset so it prints nothing, and `-` keeps it safe under `set -u`.
__marshell_ps0='${__marshell_nil[__marshell_ran=1]-}\e]133;C\e\\'
__marshell_use_ps0=
if { [ "${BASH_VERSINFO[0]}" -gt 4 ] || { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -ge 4 ]; }; } && shopt -q promptvars; then
    __marshell_use_ps0=1
fi

# First in PROMPT_COMMAND: report how the last command ended, then the folder.
__marshell_prompt_start() {
    local code=$?
    __marshell_in_prompt=1
    if [ -n "$__marshell_ran" ]; then
        printf '\033]133;D;%s\033\\' "$code"
    else
        printf '\033]133;D\033\\'
    fi
    __marshell_ran=
    printf '\033]7;file://%s%s\033\\' "${HOSTNAME:-}" "$PWD"
    return $code
}

# Last in PROMPT_COMMAND: mark the prompt start again if something replaced PS1.
__marshell_prompt_end() {
    local code=$?
    case "$PS1" in
        *'133;A'*) ;;
        *) PS1='\[\033]133;A\033\\\]'"$PS1" ;;
    esac
    if [ -n "$__marshell_use_ps0" ]; then
        case "${PS0:-}" in
            *'133;C'*) ;;
            *) PS0="${PS0:-}$__marshell_ps0" ;;
        esac
    fi
    __marshell_in_prompt=
    return $code
}

# The fallback for older bash or promptvars off, run by a DEBUG trap before each command: the first one after a
# prompt marks the command start. Every path ends with `: "$1"`, which hands `$_` back as the trap found it.
__marshell_preexec() {
    if [ -z "$__marshell_in_prompt" ] && [ -z "$__marshell_ran" ] && [ -z "${COMP_LINE:-}" ] \
        && [ "$BASH_COMMAND" != __marshell_prompt_start ]; then
        __marshell_ran=1
        printf '\033]133;C\033\\'
    fi
    : "$1"
}

# bash 5.1 and later run every element of an array PROMPT_COMMAND: ours go first and last. Older bash, or a string,
# keeps the one-string form.
if { [ "${BASH_VERSINFO[0]}" -gt 5 ] || { [ "${BASH_VERSINFO[0]}" -eq 5 ] && [ "${BASH_VERSINFO[1]}" -ge 1 ]; }; } \
    && [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(__marshell_prompt_start ${PROMPT_COMMAND[@]+"${PROMPT_COMMAND[@]}"} __marshell_prompt_end)
else
    PROMPT_COMMAND="__marshell_prompt_start"$'\n'"${PROMPT_COMMAND:-}"$'\n'"__marshell_prompt_end"
fi
if [ -n "$__marshell_use_ps0" ]; then
    PS0="${PS0:-}$__marshell_ps0"
# A DEBUG trap someone else set stays theirs on this path: without ours there is no "running" state. Bash before 5.1
# runs only PROMPT_COMMAND[0], so an array entry can't fake a mark there.
elif [ -z "$(trap -p DEBUG)" ]; then
    trap '__marshell_preexec "$_"' DEBUG
fi
