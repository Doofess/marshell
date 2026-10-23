; Marshell's installer hooks (tauri.conf.json: bundle.windows.nsis.installerHooks).

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
