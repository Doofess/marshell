; A program that runs one macro of Marshell's installer hooks against the folder it is given, so
; scripts/release/hooks-check.ps1 can try the hooks' waits for files in use without an install.
;
;   makensis /DCHECK=<uninstall|install> /DHOOKS=<hooks.nsh> /DOUT=<exe to write> [/DMARSHELL_HELD_WAIT_MS=<n>] hooks-check.nsi
;   <exe> /S /D=<folder>
;
; Two programs are built from it:
; - /DCHECK=uninstall deletes Marshell's program files from <folder>, as the uninstaller's last hook does
;   (MARSHELL_REMOVE_HELD_PROGRAMS);
; - /DCHECK=install runs the installer's wait (MARSHELL_WAIT_FOR_HELD_PROGRAMS), then writes the word "replaced"
;   into each program file that is in <folder>, standing in for the installer's `File` lines. A wait that stops
;   the install aborts before that, so the files keep what they held.
;
; Never give either one an install folder.
Unicode true
!include LogicLib.nsh
!define MAINBINARYNAME "marshell-app"
!include "${HOOKS}"

; Tauri's template declares this; the install check's wait reads it.
Var PassiveMode

Name "Marshell hooks check"
OutFile "${OUT}"
RequestExecutionLevel user
SilentInstall silent

; What `File` does to a program that is there: it replaces it, read-only or not. A file that can't be opened
; for writing is left as it was.
!macro CHECK_WRITE_PROGRAM file
  ${If} ${FileExists} "$INSTDIR\${file}"
    SetFileAttributes "$INSTDIR\${file}" NORMAL
    ClearErrors
    FileOpen $R0 "$INSTDIR\${file}" w
    ${IfNot} ${Errors}
      FileWrite $R0 "replaced"
      FileClose $R0
    ${EndIf}
  ${EndIf}
!macroend

Section
  ${If} $INSTDIR == ""
    Abort
  ${EndIf}
  !if "${CHECK}" == "uninstall"
    !insertmacro MARSHELL_REMOVE_HELD_PROGRAMS
  !else if "${CHECK}" == "install"
    !insertmacro MARSHELL_WAIT_FOR_HELD_PROGRAMS
    !insertmacro MARSHELL_EACH_PROGRAM CHECK_WRITE_PROGRAM
  !else
    !error "hooks-check.nsi needs /DCHECK=uninstall or /DCHECK=install"
  !endif
SectionEnd
