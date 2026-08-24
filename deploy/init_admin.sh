#!/bin/bash
cd ~/HyperBackupX/control
GOPROXY=https://goproxy.cn,direct go run -exec "" <<'GOEOF'
GOEOF

# Generate bcrypt hash for admin password
HASH=$(cd ~/HyperBackupX/control && GOPROXY=https://goproxy.cn,direct go run -v - <<'GOEOF' 2>/dev/null
package main
import (
    "fmt"
    "golang.org/x/crypto/bcrypt"
)
func main() {
    h, _ := bcrypt.GenerateFromPassword([]byte("admin123"), bcrypt.DefaultCost)
    fmt.Print(string(h))
}
GOEOF
)

echo "Bcrypt hash generated: ${HASH:0:20}..."

sudo -u postgres psql -d hbx_control <<SQL
CREATE TABLE IF NOT EXISTS users (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    auth_source TEXT NOT NULL DEFAULT 'local',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS roles (
    role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role_name TEXT NOT NULL UNIQUE,
    permissions JSONB NOT NULL DEFAULT '[]'::jsonb
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID NOT NULL REFERENCES users(user_id),
    role_id UUID NOT NULL REFERENCES roles(role_id),
    PRIMARY KEY (user_id, role_id)
);

INSERT INTO roles (role_name, permissions) 
VALUES ('admin', '["*"]'::jsonb)
ON CONFLICT (role_name) DO NOTHING;

INSERT INTO users (username, display_name, password_hash, auth_source, status)
VALUES ('admin', 'Administrator', '$HASH', 'local', 'active')
ON CONFLICT (username) DO UPDATE SET password_hash = '$HASH';

INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id FROM users u, roles r
WHERE u.username = 'admin' AND r.role_name = 'admin'
ON CONFLICT DO NOTHING;

SELECT username, display_name, auth_source, status FROM users;
SQL

echo "Admin user created: admin / admin123"