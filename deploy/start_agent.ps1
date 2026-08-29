$env:HBX_AGENT_CP_URL = 'http://192.168.2.3:8080'
$env:HBX_AGENT_BADOU_GRPC = 'http://192.168.2.3:9090'
$env:HBX_BADOU_JWT = 'eyJhbGciOiAiSFMyNTYiLCAidHlwIjogIkpXVCJ9.eyJzdWIiOiAiYWdlbnQiLCAicm9sZSI6ICJhZG1pbiIsICJleHAiOiAxNzg4MDczMTc4LCAiaWF0IjogMTc4Nzk4Njc3OH0.-rSmydB-Uq4DNQXJHBpOq6fDITgdaGYrlvWMaXfgTr8'

Start-Process -FilePath 'C:\Users\dell\hbx-agent\hbx-agent.exe' `
    -WorkingDirectory 'C:\Users\dell\hbx-agent' `
    -RedirectStandardOutput 'C:\Users\dell\hbx-agent\agent.log' `
    -RedirectStandardError 'C:\Users\dell\hbx-agent\agent.err' `
    -WindowStyle Hidden -PassThru | Select-Object Id, ProcessName
