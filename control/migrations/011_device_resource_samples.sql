-- G15-02: Device resource samples table for persisting heartbeat resource metrics
CREATE TABLE IF NOT EXISTS device_resource_samples (
    id SERIAL PRIMARY KEY,
    agent_id UUID NOT NULL,
    collected_at TIMESTAMP NOT NULL,
    startup_time_ms BIGINT NOT NULL DEFAULT 0,
    rss_bytes BIGINT NOT NULL DEFAULT 0,
    cpu_usage_percent DOUBLE PRECISION NOT NULL DEFAULT 0,
    io_read_bytes BIGINT NOT NULL DEFAULT 0,
    io_write_bytes BIGINT NOT NULL DEFAULT 0,
    network_rx_bytes BIGINT NOT NULL DEFAULT 0,
    network_tx_bytes BIGINT NOT NULL DEFAULT 0,
    backup_throughput_mbps DOUBLE PRECISION NOT NULL DEFAULT 0,
    peak_memory_bytes BIGINT NOT NULL DEFAULT 0,
    avg_memory_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_device_resource_samples_agent_time
    ON device_resource_samples (agent_id, collected_at DESC);

-- Resource target values (frozen after measurement phase)
CREATE TABLE IF NOT EXISTS device_resource_targets (
    id SERIAL PRIMARY KEY,
    target_name VARCHAR(64) NOT NULL UNIQUE,
    target_value DOUBLE PRECISION NOT NULL,
    unit VARCHAR(32) NOT NULL,
    frozen BOOLEAN NOT NULL DEFAULT FALSE,
    frozen_at TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

INSERT INTO device_resource_targets (target_name, target_value, unit) VALUES
    ('startup_time_ms', 0, 'ms'),
    ('rss_bytes', 0, 'bytes'),
    ('cpu_usage_percent', 0, 'percent'),
    ('io_read_bytes', 0, 'bytes'),
    ('io_write_bytes', 0, 'bytes'),
    ('network_rx_bytes', 0, 'bytes'),
    ('network_tx_bytes', 0, 'bytes'),
    ('backup_throughput_mbps', 0, 'mbps'),
    ('peak_memory_bytes', 0, 'bytes')
ON CONFLICT (target_name) DO NOTHING;