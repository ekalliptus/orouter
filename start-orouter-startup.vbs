Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "cmd /c cd /d C:\dev\orouter && node custom-server.js --port 20128", 0, False
