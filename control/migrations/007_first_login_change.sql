-- G10-04: First login password change
ALTER TABLE users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_changed_at TIMESTAMPTZ;

-- Set existing admin to require password change on next login
UPDATE users SET must_change_password = true WHERE username = 'admin';