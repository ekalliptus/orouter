' ORouter one-click starter — the desktop ORouter.lnk points here.
' Starts (hidden): the Headroom compression proxy via its guarded launcher,
' then the API server. The login watchdog keeps the server alive afterward.

Set shell = CreateObject("WScript.Shell")
Set wmi = GetObject("winmgmts:{impersonationLevel=impersonate}!\\.\root\cimv2")

' Headroom first — the server's chat path calls it per request. Its launcher
' has its own singleton guard, so this is safe when it already runs.
Set proc = wmi.Get("Win32_Process")
proc.Create "wscript.exe //nologo ""C:\dev\orouter\start-headroom.vbs""", null, null, pid

' Server, hidden — same path as before.
shell.Run "powershell.exe -ExecutionPolicy Bypass -NoProfile -WindowStyle Minimized -File ""C:\dev\orouter\start-orouter.ps1""", 0, False
