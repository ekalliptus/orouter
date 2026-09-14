' ORouter watchdog — launched detached via WMI.
' Polls the server every 30s; relaunches it if the health check fails.
' Logs to %TEMP%\orouter-watchdog.log.

Const APP_DIR = "C:\dev\orouter"
Const PORT = 20128
Const CHECK_INTERVAL_MS = 30000
Const LOG_FILE = "orouter-watchdog.log"
Const SERVER_LOG = "orouter-server.log"

Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

Sub LogLine(msg)
    On Error Resume Next
    Dim f
    Set f = fso.OpenTextFile(shell.ExpandEnvironmentStrings("%TEMP%") & "\" & LOG_FILE, 8, True)
    f.WriteLine Now & " " & msg
    f.Close
End Sub

Function ServerAlive()
    On Error Resume Next
    Dim req
    Set req = CreateObject("MSXML2.ServerXMLHTTP")
    req.open "GET", "http://127.0.0.1:" & PORT & "/api/version", False
    req.setRequestHeader "Cache-Control", "no-cache"
    req.send
    ServerAlive = (req.status = 200)
End Function

Sub LaunchServer()
    Dim cmd
    cmd = "cmd /c cd /d " & APP_DIR & " && node custom-server.js --port " & PORT & _
          " >> """ & shell.ExpandEnvironmentStrings("%TEMP%") & "\" & SERVER_LOG & """ 2>&1"
    ' 0 = hidden window, False = don't wait
    shell.Run cmd, 0, False
End Sub

LogLine "watchdog started (pid " & GetObject("winmgmts:root\cimv2:Win32_Process=@").ProcessId & ")"

Do While True
    If ServerAlive() Then
        ' healthy
    Else
        LogLine "server down -> relaunching"
        LaunchServer
        WScript.Sleep 12000
        If ServerAlive() Then
            LogLine "relaunch OK"
        Else
            LogLine "relaunch FAILED (still down)"
        End If
    End If
    WScript.Sleep CHECK_INTERVAL_MS
Loop
