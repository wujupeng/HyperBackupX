@echo off
REM HyperBackup X Agent - Windows Deployment Script
REM
REM Usage: start_agent.bat
REM
REM Prerequisites:
REM - hbx-agent.exe in the same directory
REM - Control Plane running at 192.168.2.3:8080
REM - badou-server running at 192.168.2.3:9090
REM - Python 3 installed (for JWT generation)

set HBX_AGENT_CP_URL=http://192.168.2.3:8080
set HBX_AGENT_BADOU_GRPC=http://192.168.2.3:9090

REM Generate JWT token
python -c "import hmac,hashlib,base64,json,time; secret=b'phase21-test'; header={'alg':'HS256','typ':'JWT'}; claims={'sub':'agent','role':'admin','exp':int(time.time())+86400,'iat':int(time.time())}; hb=base64.urlsafe_b64encode(json.dumps(header).encode()).rstrip(b'='); pb=base64.urlsafe_b64encode(json.dumps(claims).encode()).rstrip(b'='); si=hb+b'.'+pb; sig=hmac.new(secret,si,hashlib.sha256).digest(); sb=base64.urlsafe_b64encode(sig).rstrip(b'='); print((hb+b'.'+pb+b'.'+sb).decode())" > %TEMP%\hbx_jwt.txt
set /p HBX_BADOU_JWT=<%TEMP%\hbx_jwt.txt

echo Starting HBX Agent...
echo Control Plane: %HBX_AGENT_CP_URL%
echo Badou gRPC: %HBX_AGENT_BADOU_GRPC%

hbx-agent.exe