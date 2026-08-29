-- G11-03: Add chunking_profile column to jobs table
-- Supports Small (256KB), Standard (512KB), Large (1MB), Adaptive (4MB+) chunking strategies

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS chunking_profile VARCHAR(16) NOT NULL DEFAULT 'Standard';

-- Add comment for documentation
COMMENT ON COLUMN jobs.chunking_profile IS 'Chunking strategy: Small=256KB, Standard=512KB, Large=1MB, Adaptive=size-based';