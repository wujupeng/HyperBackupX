#!/bin/bash
sudo systemctl start postgresql
sudo -u postgres psql -c "CREATE USER hbx WITH PASSWORD 'hbx_dev_pwd';" 2>/dev/null || true
sudo -u postgres psql -c "CREATE DATABASE hbx_control OWNER hbx;" 2>/dev/null || true
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE hbx_control TO hbx;" 2>/dev/null || true
echo 'PostgreSQL configured'
sudo systemctl status postgresql | head -3