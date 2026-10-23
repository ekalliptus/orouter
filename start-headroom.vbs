' Headroom proxy launcher — started by Headroom-AutoStart.vbs in the Startup
' folder. WMI-created so it detaches from the launcher's process tree.
' Singleton: skips when any headroom.exe already runs (proxy takes ~15s to
' bind 8787, so process presence is a safer guard than probing the port).

Const HEADROOM_EXE = "C:\Users\ekalliptus\AppData\Roaming\Python\Python314\Scripts\headroom.exe"

Set wmi = GetObject("winmgmts:{impersonationLevel=impersonate}!\\.\root\cimv2")

Set procs = wmi.ExecQuery("SELECT ProcessId FROM Win32_Process WHERE Name='headroom.exe'")
If procs.Count > 0 Then
  WScript.Quit
End If

Set proc = wmi.Get("Win32_Process")
proc.Create """" & HEADROOM_EXE & """ proxy --port 8787", null, null, pid
