; Marshell's installer hooks (tauri.conf.json: bundle.windows.nsis.installerHooks).

; winget runs an installed program's QuietUninstallString when it has one. Tauri writes only
; UninstallString, which opens the uninstaller's window (PLAN §9, slice 9a decision 14).
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${UNINSTKEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
!macroend

; Closes a running Marshell, then waits until its programs have ended, before any file is copied
; (issue #214; PLAN §9, slice 9d).
;
; Tauri's installer closes the app with CheckIfAppIsRunning and copies the files straight after. The
; programs Marshell started end a moment later than the app, and a file that is in use can't be replaced:
; the installer skipped it and still exited 0, so an update could leave an old OpenConsole.exe beside a new
; conpty.dll. This is the template's first hook, so it closes the app with the template's own macro and
; waits; the template's check that comes next then finds nothing left to close. Nothing else is closed or
; killed here. When the wait gives up, nothing has been copied yet and the install stops.
!macro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  !insertmacro MARSHELL_WAIT_FOR_HELD_PROGRAMS
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

; The two waits below share these limits, so the uninstaller and the installer give up at the same time.
; MARSHELL_HELD_WAIT_MS counts the pauses, not the time the tries take, so a slow disk makes a wait longer.
; The copy the person reads says the same time in seconds, and takes it from the limit; a build can lower
; the limit (scripts/release/hooks-check.ps1 does), and the copy follows. MARSHELL_HELD_EXIT is the exit code
; of an install that stops because a program is still running: the winget manifest maps it to fileInUse
; (scripts/release/winget-manifest.mjs), while the template's own aborts keep exit code 2.
!ifndef MARSHELL_HELD_WAIT_MS
  !define MARSHELL_HELD_WAIT_MS 20000
!endif
!define MARSHELL_HELD_STEP_MS 250
!define /math MARSHELL_HELD_WAIT_S ${MARSHELL_HELD_WAIT_MS} / 1000
!define MARSHELL_HELD_EXIT 32

; One line for each program the installer puts in the install folder, in the order it copies them. Both waits
; go through this list, so a program can't be in one and missing from the other
; (scripts/release/hooks.test.ts keeps it in step with the bundle). `do` is the name of a macro that takes
; the file name.
!macro MARSHELL_EACH_PROGRAM do
  !insertmacro ${do} "${MAINBINARYNAME}.exe"
  !insertmacro ${do} "marshell.exe"
  !insertmacro ${do} "marshell-hook.exe"
  !insertmacro ${do} "marshell-host.exe"
  !insertmacro ${do} "OpenConsole.exe"
  !insertmacro ${do} "conpty.dll"
!macroend

; Deletes Marshell's programs that were still in use when the uninstaller first tried, once they are free
; (issue #212; PLAN §9, slice 9b).
;
; The uninstaller closes a running Marshell and deletes the program files straight after. The programs
; Marshell started (OpenConsole.exe for each terminal, marshell-host.exe, a hook) end a moment later than
; the app itself, whether it was closed here, was killed or crashed, and a program that is running can't
; have its file deleted: the uninstaller left it, and the install folder with it, without a word. Tauri's
; installer has no hook between closing the app and deleting the files, so this runs first in the last hook,
; a moment after that first try, and tries again every quarter of a second for up to MARSHELL_HELD_WAIT_MS.
; After that it gives up, and what is still in use stays, as before: an administrator session's
; marshell-host.exe isn't closed by anything here.
;
; It names every program the installer puts in the install folder, and nothing else in the folder is
; touched or waited for (scripts/release/hooks.test.ts keeps the list in step with the bundle;
; scripts/release/hooks-check.ps1 tries the wait and the limit).

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
    !insertmacro MARSHELL_EACH_PROGRAM MARSHELL_DELETE_PROGRAM
    ${If} $R9 = 0
    ${OrIf} $R8 >= ${MARSHELL_HELD_WAIT_MS}
      ${ExitDo}
    ${EndIf}
    ${If} $R8 = 0
      DetailPrint "Waiting for Marshell's programs to end (up to ${MARSHELL_HELD_WAIT_S} seconds)"
    ${EndIf}
    Sleep ${MARSHELL_HELD_STEP_MS}
    IntOp $R8 $R8 + ${MARSHELL_HELD_STEP_MS}
  ${Loop}
  ; The folder went with the last file the first time round; this is for the files that went just now.
  RMDir "$INSTDIR"
  Pop $R9
  Pop $R8
!macroend

; Sets $R9 to the program's name when it is there and still in use, unless $R9 already names one. "In use" is
; what `File` would fail on: opening the file for writing, with every kind of sharing allowed, fails with a
; sharing or lock violation (32 or 33), which is what a running program gives and what a file another program
; holds gives. A file that is missing is not waited for (a first install, or a folder with only some of the
; programs), and a file that is read-only gives 5, which `File` replaces by clearing the flag, so that isn't in
; use either. The handle decides first: a handle that opened is closed and nothing else is read, and the error
; is read only when there is none, because a successful call can leave an old error behind.
!macro MARSHELL_PROBE_PROGRAM file
  ${If} $R9 == ""
  ${AndIf} ${FileExists} "$INSTDIR\${file}"
    System::Call 'kernel32::CreateFileW(w "$INSTDIR\${file}", i 0x40000000, i 7, p 0, i 3, i 0x80, p 0) p.R7 ?e'
    Pop $R6
    ${If} $R7 P<> -1
      System::Call 'kernel32::CloseHandle(p R7)'
    ${ElseIf} $R6 = 32
    ${OrIf} $R6 = 33
      StrCpy $R9 "${file}"
    ${EndIf}
  ${EndIf}
!macroend

; Waits until none of Marshell's programs in the install folder is in use, trying every
; MARSHELL_HELD_STEP_MS for up to MARSHELL_HELD_WAIT_MS of pauses (issue #214; PLAN §9, slice 9d). It looks only
; at those files, never at a process by name: Windows Terminal has an OpenConsole.exe of its own that isn't
; ours to wait for.
;
; At the limit it stops the install, before anything is copied:
; - a silent or passive install ends with exit code MARSHELL_HELD_EXIT and says why on the standard output of
;   the console that started it, when there is one (winget may or may not show it);
; - one with the installer's window asks Retry or Cancel. Retry waits the same time again; Cancel stops as above.
; The words differ when the main program file is gone by then: a windowed upgrade has "uninstall before
; installing" checked, and that removed the earlier version first, so the person is told it is not installed.
; It uses $R4 to $R9 and gives them back.
!macro MARSHELL_WAIT_FOR_HELD_PROGRAMS
  Push $R4
  Push $R5
  Push $R6
  Push $R7
  Push $R8
  Push $R9
  StrCpy $R8 0
  ${Do}
    StrCpy $R9 ""
    !insertmacro MARSHELL_EACH_PROGRAM MARSHELL_PROBE_PROGRAM
    ${If} $R9 == ""
      ${ExitDo}
    ${EndIf}
    ${If} $R8 >= ${MARSHELL_HELD_WAIT_MS}
      ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
        StrCpy $R4 "Retry waits for it again. Cancel stops the installer and changes nothing."
        StrCpy $R5 "Marshell wasn't updated: $R9 was still running ${MARSHELL_HELD_WAIT_S} seconds after Marshell closed. Nothing was changed. Run the update again."
      ${Else}
        StrCpy $R4 "Retry waits for it again. Cancel stops the installer; the earlier version has already been removed."
        StrCpy $R5 "Marshell wasn't installed: $R9 was still running ${MARSHELL_HELD_WAIT_S} seconds after Marshell closed. The earlier version had already been removed. Run the installer again."
      ${EndIf}
      ${If} ${Silent}
      ${OrIf} $PassiveMode = 1
        System::Call 'kernel32::AttachConsole(i -1)'
        System::Call 'kernel32::GetStdHandle(i -11) p.R6'
        ${If} $R6 P<> 0
        ${AndIf} $R6 P<> -1
          FileWrite $R6 "$R5$\r$\n"
        ${EndIf}
        SetErrorLevel ${MARSHELL_HELD_EXIT}
        Abort "$R5"
      ${EndIf}
      ${IfNot} ${Cmd} `MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Marshell has closed, but its program $R9 is still running, so the installer can't replace it.$\n$\n$R4" /SD IDCANCEL IDRETRY`
        SetErrorLevel ${MARSHELL_HELD_EXIT}
        Abort "$R5"
      ${EndIf}
      StrCpy $R8 0
    ${EndIf}
    ${If} $R8 = 0
      DetailPrint "Waiting for Marshell's programs to end (up to ${MARSHELL_HELD_WAIT_S} seconds)"
    ${EndIf}
    Sleep ${MARSHELL_HELD_STEP_MS}
    IntOp $R8 $R8 + ${MARSHELL_HELD_STEP_MS}
  ${Loop}
  Pop $R9
  Pop $R8
  Pop $R7
  Pop $R6
  Pop $R5
  Pop $R4
!macroend
