-- G11-04: Add restore path selection parameters to restore_jobs table
-- Supports source_path_prefix (optional), target_path, overwrite_policy (skip|overwrite|rename)

ALTER TABLE restore_jobs ADD COLUMN IF NOT EXISTS source_path_prefix VARCHAR(1024);
ALTER TABLE restore_jobs ADD COLUMN IF NOT EXISTS target_path VARCHAR(1024);
ALTER TABLE restore_jobs ADD COLUMN IF NOT EXISTS overwrite_policy VARCHAR(16) NOT NULL DEFAULT 'skip';

-- Add comments for documentation
COMMENT ON COLUMN restore_jobs.source_path_prefix IS 'Optional path prefix to filter files for selective restore';
COMMENT ON COLUMN restore_jobs.target_path IS 'Target directory for restore operation';
COMMENT ON COLUMN restore_jobs.overwrite_policy IS 'Conflict handling: skip, overwrite, or rename';