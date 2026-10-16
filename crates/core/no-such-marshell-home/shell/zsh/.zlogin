# Marshell: loads your .zlogin, the last startup file, then hands ZDOTDIR back. See .zshenv here.

ZDOTDIR=$__marshell_user
[[ -r $ZDOTDIR/.zlogin ]] && source "$ZDOTDIR/.zlogin"
__marshell_user=$ZDOTDIR
__marshell_restore
