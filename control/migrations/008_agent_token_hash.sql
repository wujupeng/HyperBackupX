-- G10-09: Agent token hash for revocation
ALTER TABLE devices ADD COLUMN IF NOT EXISTS agent_token_hash VARCHAR(64);