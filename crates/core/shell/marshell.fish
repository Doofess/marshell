# Marshell shell integration for fish. Marshell runs this with -C, after fish has read your own
# config. Nothing of yours is edited.
# It marks each prompt and command (OSC 133) and reports the folder (OSC 7), so Marshell can
# show whether a command is running and how the last one ended.

if not set -q MARSHELL_SHELL_INTEGRATION
    set -g MARSHELL_SHELL_INTEGRATION 1
    set -g __marshell_status ''

    function __marshell_postexec --on-event fish_postexec
        set -g __marshell_status $status
    end

    function __marshell_preexec --on-event fish_preexec
        printf '\e]133;C\e\\'
    end

    function __marshell_prompt --on-event fish_prompt
        if test -n "$__marshell_status"
            printf '\e]133;D;%s\e\\' $__marshell_status
        else
            printf '\e]133;D\e\\'
        end
        set -g __marshell_status ''
        printf '\e]7;file://%s%s\e\\' $hostname $PWD
        printf '\e]133;A\e\\'
    end
end
