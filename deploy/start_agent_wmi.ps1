$result = ([WMIClass]"\\.\ROOT\cimv2:Win32_Process").Create("cmd /c C:\Users\dell\hbx-agent\start_agent_windows.bat")
Write-Output "ProcessId: $($result.ProcessId)"
Write-Output "ReturnValue: $($result.ReturnValue)"