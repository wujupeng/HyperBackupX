#!/bin/bash
set -euo pipefail

# Generate strong random secrets for HyperBackup X Control Plane
# Usage: sudo ./gen_control_env.sh
# Output: /etc/hbx/control.env (root:root 0600)

ENV_FILE="/etc/hbx/control.env"

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: must run as root" >&2
    exit 1
fi

JWT_SECRET=$(openssl rand -base64 48)
DB_PASSWORD=$(openssl rand -base64 32)
ADMIN_PASSWORD=$(openssl rand -base64 24)
AGENT_TOKEN_PEPPER=$(openssl rand -base64 48)

cat > "$ENV_FILE" << EOF
HBX_JWT_SECRET=$JWT_SECRET
HBX_DB_PASSWORD=$DB_PASSWORD
HBX_ADMIN_PASSWORD=$ADMIN_PASSWORD
HBX_AGENT_TOKEN_PEPPER=$AGENT_TOKEN_PEPPER
HBX_PG_SSLMODE=require
EOF

chmod 0600 "$ENV_FILE"
chown root:root "$ENV_FILE"

# Sync DB password to PostgreSQL
PG_BIN=$(which psql 2>/dev/null || echo "/usr/lib/postgresql/*/bin/psql")
if command -v psql &>/dev/null; then
    PGPASSWORD="$DB_PASSWORD" psql -h 127.0.0.1 -p 5433 -U hbx -d hbx -c "ALTER USER hbx PASSWORD '$DB_PASSWORD';" 2>/dev/null || true
fi

echo "Secrets generated and written to $ENV_FILE"
echo "DB password synced to PostgreSQL (if available)"