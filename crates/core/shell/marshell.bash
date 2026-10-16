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
    __marshell_in_prompt=
    return $code
}

# Runs before each command: the first one after a prompt marks the command start.
__marshell_preexec() {
    [ -n "$__marshell_in_prompt" ] && return
    [ -n "$__marshell_ran" ] && return
    [ -n "${COMP_LINE:-}" ] && return
    [ "$BASH_COMMAND" = __marshell_prompt_start ] && return
    __marshell_ran=1
    printf '\033]133;C\033\\'
}

PROMPT_COMMAND="__marshell_prompt_start"$'\n'"${PROMPT_COMMAND:-}"$'\n'"__marshell_prompt_end"
# A DEBUG trap someone else set stays theirs: without ours there is no "running" state.
if [ -z "$(trap -p DEBUG)" ]; then
    trap '__marshell_preexec' DEBUG
fi
