; Marshell's installer hooks (tauri.conf.json: bundle.windows.nsis.installerHooks).

; winget runs an installed program's QuietUninstallString when it has one. Tauri writes only
; UninstallString, which opens the uninstaller's window (PLAN §9, slice 9a decision 14).
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${UNINSTKEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
!macroend

; Takes Marshell's hooks and its skill out of every agent CLI's files, and removes ~/.marshell/bin, before
; the program files go: `marshell uninstall --yes` (PLAN §9, slice 9a decisions 7, 9 and 14). Its exit
; code is read and not acted on. When a CLI's file can't be changed the uninstall still goes on, and
; ~/.marshell/bin stays so the hooks left behind still find their bridge. The command bounds its own WSL
; calls; the timeout here (two minutes with no output) is for a file share that hangs, so the uninstall
; still ends.
;
; An uninstall that is part of an install keeps the hooks. There are two:
; - the updater's, which passes /UPDATE;
; - the installer's "uninstall before installing", which runs this uninstaller in place
;   (`uninstall.exe _?=<folder>`), so it runs from the install folder. An uninstall started any other way
;   (Add/Remove Programs, winget) first hands over to a copy of itself in the temp folder.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $EXEDIR != $INSTDIR
  ${AndIf} ${FileExists} "$INSTDIR\marshell.exe"
    DetailPrint "Taking Marshell's hooks out of the agent CLIs"
    nsExec::ExecToLog /TIMEOUT=120000 '"$INSTDIR\marshell.exe" uninstall --yes'
    Pop $R7
  ${EndIf}
!macroend

; Removes the marshell:// link when it points at the exe being uninstalled. The link is registered by the
; Notifications switch under HKCU, not by the installer, so a key that points anywhere else (a dev build,
; another install path) is left alone. An update runs the old uninstaller too; the link stays then.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ReadRegStr $R7 HKCU "Software\Classes\marshell\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey HKCU "Software\Classes\marshell"
    ${EndIf}
  ${EndIf}
!macroend
