Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "powershell.exe -ExecutionPolicy Bypass -NoProfile -WindowStyle Minimized -File ""C:\dev\orouter\start-orouter.ps1""", 0, False
