# Marshell: loads your .zprofile, then comes back. See .zshenv here.

ZDOTDIR=$__marshell_user
[[ -r $ZDOTDIR/.zprofile ]] && source "$ZDOTDIR/.zprofile"
__marshell_user=$ZDOTDIR
ZDOTDIR=$__marshell_shim
