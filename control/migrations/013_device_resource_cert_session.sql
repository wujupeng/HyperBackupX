-- G17-G20 DB-01.1: Add cert_session_id to device_resource_samples for Soak attribution
ALTER TABLE device_resource_samples
    ADD COLUMN IF NOT EXISTS cert_session_id VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_device_resource_samples_cert_session
    ON device_resource_samples (cert_session_id, collected_at DESC)
    WHERE cert_session_id IS NOT NULL;