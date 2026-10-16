# Marshell starts zsh with ZDOTDIR pointing here, so zsh reads these files instead of yours.
# Each one loads your file of the same name, with ZDOTDIR set to your folder, and comes back.
# Nothing of yours is edited.

__marshell_shim=$ZDOTDIR
__marshell_user_set=${MARSHELL_USER_ZDOTDIR:+1}
__marshell_user=${MARSHELL_USER_ZDOTDIR:-$HOME}
unset MARSHELL_USER_ZDOTDIR

# Hands ZDOTDIR back once startup is over: to your folder, or unset if you never set one.
__marshell_restore() {
    if [[ -z $__marshell_user_set && $__marshell_user == "$HOME" ]]; then
        unset ZDOTDIR
    else
        export ZDOTDIR=$__marshell_user
    fi
    unset __marshell_shim __marshell_user __marshell_user_set
    unfunction __marshell_restore
}

ZDOTDIR=$__marshell_user
[[ -r $ZDOTDIR/.zshenv ]] && source "$ZDOTDIR/.zshenv"
# Your .zshenv may have moved ZDOTDIR; the later files then come from there.
[[ $ZDOTDIR != "$__marshell_user" ]] && __marshell_user_set=1
__marshell_user=$ZDOTDIR

if [[ -o interactive || -o login ]]; then
    ZDOTDIR=$__marshell_shim
else
    __marshell_restore
fi
