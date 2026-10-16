# Marshell: loads your .zshrc, then Marshell's prompt marks. See .zshenv here.

ZDOTDIR=$__marshell_user
[[ -r $ZDOTDIR/.zshrc ]] && source "$ZDOTDIR/.zshrc"
__marshell_user=$ZDOTDIR
source "${__marshell_shim:h}/marshell.zsh"

if [[ -o login ]]; then
    ZDOTDIR=$__marshell_shim
else
    __marshell_restore
fi
