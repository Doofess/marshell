; A program that runs one macro of Marshell's installer hooks against the folder it is given, so
; scripts/release/hooks-check.ps1 can try the hooks' wait for files in use without an install.
;
;   makensis /DHOOKS=<hooks.nsh> /DOUT=<exe to write> [/DMARSHELL_HELD_WAIT_MS=<n>] hooks-check.nsi
;   <exe> /S /D=<folder>
;
; It deletes Marshell's program files from <folder>, as the uninstaller's last hook does. Never give it
; an install folder.
Unicode true
!include LogicLib.nsh
!define MAINBINARYNAME "marshell-app"
!include "${HOOKS}"

Name "Marshell hooks check"
OutFile "${OUT}"
RequestExecutionLevel user
SilentInstall silent

Section
  ${If} $INSTDIR == ""
    Abort
  ${EndIf}
  !insertmacro MARSHELL_REMOVE_HELD_PROGRAMS
SectionEnd
