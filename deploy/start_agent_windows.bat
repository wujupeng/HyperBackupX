@echo off
REM HyperBackup X Agent - Windows Startup Script
REM Sets environment variables and starts the agent

set HBX_AGENT_CP_URL=http://192.168.2.3:8080
set HBX_AGENT_BADOU_GRPC=http://192.168.2.3:9090
set HBX_BADOU_JWT=eyJhbGciOiAiSFMyNTYiLCAidHlwIjogIkpXVCJ9.eyJzdWIiOiAiYWdlbnQiLCAicm9sZSI6ICJhZG1pbiIsICJleHAiOiAxNzg4MDczMTc4LCAiaWF0IjogMTc4Nzk4Njc3OH0.-rSmydB-Uq4DNQXJHBpOq6fDITgdaGYrlvWMaXfgTr8

cd /d C:\Users\dell\hbx-agent
hbx-agent.exe >> C:\Users\dell\hbx-agent\agent.log 2>&1
