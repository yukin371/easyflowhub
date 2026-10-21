; NSIS installer hooks for EasyFlowHub.
;
; Wired via bundle.windows.nsis.installerHooks in tauri.conf.json.
;
; Problem: EasyFlowHub runs a resident Go sidecar (scriptmgr.exe) that stays
; alive after the main app is closed to the tray. Windows locks a running .exe,
; so NSIS could not overwrite scriptmgr.exe during upgrade and aborted with
; "Error opening file for writing".
;
; Fix: terminate the running processes before the installer copies files. The
; main app must be killed first so it cannot respawn the sidecar; then the
; sidecar itself. A short pause lets the OS release file handles before the
; File operations run. nsExec is bundled with NSIS, so this needs no extra
; plugin. taskkill exits non-zero when a process is not running, which is
; expected and ignored.
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'taskkill /F /IM easyflowhub.exe'
  Pop $0
  nsExec::Exec 'taskkill /F /IM scriptmgr.exe'
  Pop $0
  ; Give Windows a moment to release the locked executable handles.
  Sleep 700
!macroend
