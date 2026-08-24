#!/bin/bash
cd ~/HyperBackupX/control
export HBX_CONTROL_ADDR=:8080
export HBX_DB_DSN="postgres://hbx:hbx_dev_pwd@localhost:5432/hbx_control?sslmode=disable"
nohup ./hbx-control > hbx-control.log 2>&1 &
echo "Control Plane PID: $!"
sleep 2
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/api/v1/health 2>/dev/null || echo "Control Plane starting..."

cd ~/HyperBackupX/web
nohup npx vite preview --port 3000 --host > vite-preview.log 2>&1 &
echo "Web Preview PID: $!"
sleep 3
curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/ 2>/dev/null || echo "Web starting..."