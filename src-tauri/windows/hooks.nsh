; Marshell's installer hooks (tauri.conf.json: bundle.windows.nsis.installerHooks).

; winget runs an installed program's QuietUninstallString when it has one. Tauri writes only
; UninstallString, which opens the uninstaller's window (PLAN §9, slice 9a decision 14).
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${UNINSTKEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
!macroend

; Takes Marshell's hooks and its skill out of every agent CLI's files, and removes ~/.marshell/bin:
; `marshell uninstall --yes` (PLAN §9, slice 9a decisions 7, 9 and 14).
;
; It runs after the program files are gone, not before: the uninstaller first asks to close a running
; Marshell, and the person can still cancel there. So this macro only keeps a copy of marshell.exe in the
; uninstaller's own temp folder, and NSIS_HOOK_POSTUNINSTALL runs that copy.
;
; An uninstall that is part of an install keeps the hooks, and no copy is made. There are two:
; - the updater's, which passes /UPDATE;
; - the installer's "uninstall before installing", which runs this uninstaller in place
;   (`uninstall.exe _?=<folder>`), so it runs from the install folder. An uninstall started any other way
;   (Add/Remove Programs, winget) first hands over to a copy of itself in the temp folder.
!macro NSIS_HOOK_PREUNINSTALL
  InitPluginsDir
  Delete "$PLUGINSDIR\marshell.exe"
  ${If} $UpdateMode <> 1
  ${AndIf} $EXEDIR != $INSTDIR
  ${AndIf} ${FileExists} "$INSTDIR\marshell.exe"
    CopyFiles /SILENT "$INSTDIR\marshell.exe" "$PLUGINSDIR\marshell.exe"
  ${EndIf}
!macroend

; Runs the unhook with the copy kept above. Its exit code is read and not acted on. When a CLI's file can't
; be changed the uninstall still ends, and ~/.marshell/bin stays so the hooks left behind still find their
; bridge. The command bounds its own WSL calls; the timeout here (two minutes with no output) is for a
; file share that hangs.
;
; Then removes the marshell:// link when it points at the exe being uninstalled. The link is registered by
; the Notifications switch under HKCU, not by the installer, so a key that points anywhere else (a dev
; build, another install path) is left alone. An uninstall that is part of an install keeps the link, as
; it keeps the hooks.
!macro NSIS_HOOK_POSTUNINSTALL
  ; First, before the hooks are taken out, and for every kind of uninstall. By now the uninstall entry is gone
  ; and the unhook below can take minutes; an install that follows the uninstall may write these same files in
  ; that time, so nothing may delete them later than this. (The uninstall a newer installer starts before it
  ; installs is the installed version's own, so that install is helped only from the version with this wait on.)
  !insertmacro MARSHELL_REMOVE_HELD_PROGRAMS
  ${If} ${FileExists} "$PLUGINSDIR\marshell.exe"
    DetailPrint "Taking Marshell's hooks out of the agent CLIs"
    nsExec::ExecToLog /TIMEOUT=120000 '"$PLUGINSDIR\marshell.exe" uninstall --yes'
    Pop $R7
    Delete "$PLUGINSDIR\marshell.exe"
  ${EndIf}
  ${If} $UpdateMode <> 1
  ${AndIf} $EXEDIR != $INSTDIR
    ReadRegStr $R7 HKCU "Software\Classes\marshell\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey HKCU "Software\Classes\marshell"
    ${EndIf}
  ${EndIf}
!macroend

; Deletes Marshell's programs that were still in use when the uninstaller first tried, once they are free
; (issue #212; PLAN §9, slice 9b).
;
; The uninstaller closes a running Marshell and deletes the program files straight after. The programs
; Marshell started (OpenConsole.exe for each terminal, marshell-host.exe, a hook) end a moment later than
; the app itself, whether it was closed here, was killed or crashed, and a program that is running can't
; have its file deleted: the uninstaller left it, and the install folder with it, without a word. Tauri's
; installer has no hook between closing the app and deleting the files, so this runs first in the last hook,
; a moment after that first try, and tries again every quarter of a second for up to MARSHELL_HELD_WAIT_MS
; (it counts the pauses, not the time the tries take, so a slow disk makes it longer). After that it gives
; up, and what is still in use stays, as before: an administrator session's marshell-host.exe isn't closed
; by anything here.
;
; It names every program the installer puts in the install folder, and nothing else in the folder is
; touched or waited for (scripts/release/hooks.test.ts keeps the list in step with the bundle;
; scripts/release/hooks-check.ps1 tries the wait and the limit).
!ifndef MARSHELL_HELD_WAIT_MS
  !define MARSHELL_HELD_WAIT_MS 20000
!endif
!define MARSHELL_HELD_STEP_MS 250

; Deletes one program if it is there. Sets $R9 to 1 when it is still there afterwards.
!macro MARSHELL_DELETE_PROGRAM file
  ${If} ${FileExists} "$INSTDIR\${file}"
    Delete "$INSTDIR\${file}"
    ${If} ${FileExists} "$INSTDIR\${file}"
      StrCpy $R9 1
    ${EndIf}
  ${EndIf}
!macroend

!macro MARSHELL_REMOVE_HELD_PROGRAMS
  Push $R8
  Push $R9
  StrCpy $R8 0
  ${Do}
    StrCpy $R9 0
    !insertmacro MARSHELL_DELETE_PROGRAM "${MAINBINARYNAME}.exe"
    !insertmacro MARSHELL_DELETE_PROGRAM "marshell.exe"
    !insertmacro MARSHELL_DELETE_PROGRAM "marshell-hook.exe"
    !insertmacro MARSHELL_DELETE_PROGRAM "marshell-host.exe"
    !insertmacro MARSHELL_DELETE_PROGRAM "OpenConsole.exe"
    !insertmacro MARSHELL_DELETE_PROGRAM "conpty.dll"
    ${If} $R9 = 0
    ${OrIf} $R8 >= ${MARSHELL_HELD_WAIT_MS}
      ${ExitDo}
    ${EndIf}
    ${If} $R8 = 0
      DetailPrint "Waiting for Marshell's programs to end"
    ${EndIf}
    Sleep ${MARSHELL_HELD_STEP_MS}
    IntOp $R8 $R8 + ${MARSHELL_HELD_STEP_MS}
  ${Loop}
  ; The folder went with the last file the first time round; this is for the files that went just now.
  RMDir "$INSTDIR"
  Pop $R9
  Pop $R8
!macroend
