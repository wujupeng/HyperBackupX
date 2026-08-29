@echo off
REM HyperBackup X Agent - Windows Service Install Script
REM Creates a scheduled task that runs the agent independently of SSH sessions

REM Create the scheduled task
schtasks /create /tn "HBXAgent" /tr "cmd /c C:\Users\dell\hbx-agent\start_agent_windows.bat" /sc onstart /ru SYSTEM /f

REM Run it now
schtasks /run /tn "HBXAgent"

REM Wait a moment and check
timeout /t 3 /nobreak >nul
tasklist | findstr /i "hbx-agent"