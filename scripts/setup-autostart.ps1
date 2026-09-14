# Create ORouter autostart scheduled task (runs at user logon, restarts on failure)
$action = New-ScheduledTaskAction -Execute 'node.exe' -Argument 'custom-server.js --port 20128' -WorkingDirectory 'C:\dev\orouter'
$trigger = New-ScheduledTaskTrigger -AtLogOn
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit (New-TimeSpan -Days 3650)
Register-ScheduledTask -TaskName 'ORouter' -Action $action -Trigger $trigger -Settings $settings -Description 'ORouter AI gateway auto-start on login (port 20128)' -Force | Select-Object TaskName, State
