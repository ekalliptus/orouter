' ORouter autostart + watchdog launcher
' Entry point for the Windows Startup folder. Launches the real watchdog
' through WMI Win32_Process.Create so it fully detaches from this process
' tree and survives regardless of how the session ends.
' The watchdog: checks the server every 30s, relaunches it if dead, logs to
' %TEMP%\orouter-watchdog.log.

Const APP_DIR = "C:\dev\orouter"

Set wmi = GetObject("winmgmts:{impersonationLevel=impersonate}!\\.\root\cimv2")

' Singleton guard: skip if a watchdog is already running.
Set procs = wmi.ExecQuery("SELECT ProcessId FROM Win32_Process WHERE Name='wscript.exe' AND CommandLine LIKE '%start-orouter-startup.vbs%watch%'")
If procs.Count > 0 Then
  WScript.Quit
End If

' Detached launch — the new process is a child of WMI, not of this script,
' so it keeps running after this launcher exits.
Set proc = wmi.Get("Win32_Process")
proc.Create "wscript.exe //nologo """ & APP_DIR & "\start-orouter-watchdog.vbs""", null, null, pid
