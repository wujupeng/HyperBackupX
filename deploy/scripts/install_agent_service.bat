@echo off
REM HyperBackup X Agent Service Installation Script
REM Uses native Windows sc.exe - NO NSSM
REM
REM Usage: install_agent_service.bat [agent_exe_path]
REM
REM This script:
REM 1. Creates the service using sc.exe create (SERVICE_WIN32_OWN_PROCESS)
REM 2. Configures failure recovery (restart on crash, 3 retries, 60s reset)
REM 3. Enables delayed auto-start
REM 4. Starts the service
REM
REM NSSM is explicitly NOT used. CI checks prohibit NSSM references.

setlocal

set AGENT_NAME=HyperBackupXAgent
set AGENT_DISPLAY=HyperBackup X Agent

if "%~1"=="" (
    set AGENT_PATH=%~dp0hbx-agent.exe
) else (
    set AGENT_PATH=%~1
)

if not exist "%AGENT_PATH%" (
    echo ERROR: Agent executable not found: %AGENT_PATH%
    exit /b 1
)

echo Installing %AGENT_NAME% service...
echo Binary: %AGENT_PATH%

REM Create service as SERVICE_WIN32_OWN_PROCESS (Session 0 isolation)
REM No SERVICE_INTERACTIVE_PROCESS - ensures Session 0 isolation
sc.exe create %AGENT_NAME% binPath= "%AGENT_PATH%" start= auto DisplayName= "%AGENT_DISPLAY%"
if errorlevel 1 (
    echo ERROR: Failed to create service
    exit /b 1
)

REM Set description
sc.exe description %AGENT_NAME% "HyperBackup X backup agent service - native Windows Service"

REM Configure failure recovery:
REM - Restart on failure with 5s delay
REM - 3 restart attempts
REM - Reset failure count after 60s
sc.exe failure %AGENT_NAME% reset= 60 actions= restart/5000/restart/5000/restart/5000
if errorlevel 1 (
    echo WARNING: Failed to set failure recovery
)

REM Enable delayed auto-start
reg add "HKLM\SYSTEM\CurrentControlSet\Services\%AGENT_NAME%" /v DelayedAutostart /t REG_DWORD /d 1 /f
if errorlevel 1 (
    echo WARNING: Failed to set delayed auto-start
)

echo.
echo Service installed successfully.
echo   Name: %AGENT_NAME%
echo   Binary: %AGENT_PATH%
echo   Start: Auto (Delayed)
echo   Recovery: Restart x3 (5s delay, 60s reset)
echo   Session 0: Yes (SERVICE_WIN32_OWN_PROCESS)
echo.

REM Start the service
echo Starting service...
sc.exe start %AGENT_NAME%
if errorlevel 1 (
    echo WARNING: Failed to start service. Check event log.
) else (
    echo Service started successfully.
)

exit /b 0