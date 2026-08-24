package main

import (
	"context"
	"fmt"
	"os"

	"github.com/jackc/pgx/v5"
	"golang.org/x/crypto/bcrypt"
)

func main() {
	dsn := "postgres://hbx:hbx_dev_pwd@localhost:5432/hbx_control?sslmode=disable"
	conn, err := pgx.Connect(context.Background(), dsn)
	if err != nil {
		fmt.Fprintf(os.Stderr, "connect failed: %v\n", err)
		os.Exit(1)
	}
	defer conn.Close(context.Background())

	ctx := context.Background()

	conn.Exec(ctx, `CREATE TABLE IF NOT EXISTS users (
		user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
		username TEXT NOT NULL UNIQUE,
		display_name TEXT NOT NULL,
		password_hash TEXT NOT NULL,
		auth_source TEXT NOT NULL DEFAULT 'local',
		status TEXT NOT NULL DEFAULT 'active',
		created_at TIMESTAMPTZ NOT NULL DEFAULT now()
	)`)

	conn.Exec(ctx, `CREATE TABLE IF NOT EXISTS roles (
		role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
		role_name TEXT NOT NULL UNIQUE,
		permissions JSONB NOT NULL DEFAULT '[]'::jsonb
	)`)

	conn.Exec(ctx, `CREATE TABLE IF NOT EXISTS user_roles (
		user_id UUID NOT NULL REFERENCES users(user_id),
		role_id UUID NOT NULL REFERENCES roles(role_id),
		PRIMARY KEY (user_id, role_id)
	)`)

	conn.Exec(ctx, `INSERT INTO roles (name, is_builtin, permissions) VALUES ('admin', TRUE, '["*"]'::jsonb) ON CONFLICT (name) DO NOTHING`)

	hash, _ := bcrypt.GenerateFromPassword([]byte("admin123"), bcrypt.DefaultCost)
	_, err = conn.Exec(ctx, `INSERT INTO users (username, display_name, email, password_hash, auth_source, status) VALUES ('admin', 'Administrator', 'admin@hbx.local', $1, 'local', 'active') ON CONFLICT (username) DO UPDATE SET password_hash = $1`, string(hash))
	if err != nil {
		fmt.Fprintf(os.Stderr, "insert user failed: %v\n", err)
		os.Exit(1)
	}

	_, err = conn.Exec(ctx, `INSERT INTO user_roles (user_id, role_id) SELECT u.user_id, r.role_id FROM users u, roles r WHERE u.username = 'admin' AND r.name = 'admin' ON CONFLICT DO NOTHING`)
	if err != nil {
		fmt.Fprintf(os.Stderr, "insert role failed: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("Admin user created: admin / admin123")
}