package api

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"

	"hbx-control/internal/audit"
)

type compatTaskCommand struct {
	CommandType string                 `json:"command_type"`
	ExecutionID string                 `json:"execution_id"`
	JobID       string                 `json:"job_id"`
	RepoID      string                 `json:"repo_id"`
	BackupType  string                 `json:"backup_type"`
	Params      map[string]interface{} `json:"params"`
}

func (s *Server) getCompatPendingCommands(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT e.execution_id, e.job_id, j.repo_id, j.backup_type, j.dual_repo_mode
		FROM compat_executions e
		JOIN compat_jobs j ON e.job_id = j.job_id
		WHERE e.state = 'pending'
		ORDER BY e.started_at ASC
		LIMIT 10
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"commands": []compatTaskCommand{}})
		return
	}
	defer rows.Close()

	var commands []compatTaskCommand
	for rows.Next() {
		var execID, jobID, repoID uuid.UUID
		var backupType, dualRepoMode string
		rows.Scan(&execID, &jobID, &repoID, &backupType, &dualRepoMode)

		cmdType := "trigger_compat_backup"
		if dualRepoMode == "dual_with_consistency" {
			cmdType = "trigger_compat_backup_with_dual_check"
		}

		commands = append(commands, compatTaskCommand{
			CommandType: cmdType,
			ExecutionID: execID.String(),
			JobID:       jobID.String(),
			RepoID:      repoID.String(),
			BackupType:  backupType,
			Params: map[string]interface{}{
				"dual_repo_mode": dualRepoMode,
			},
		})

		s.pool.Exec(c.Request.Context(), `
			UPDATE compat_executions SET state = 'aligning' WHERE execution_id = $1
		`, execID)
	}
	if commands == nil {
		commands = []compatTaskCommand{}
	}
	c.JSON(http.StatusOK, gin.H{"commands": commands})
}

type compatTaskResultRequest struct {
	ExecutionID    string     `json:"execution_id" binding:"required"`
	AgentID        string     `json:"agent_id"`
	State          string     `json:"state" binding:"required"`
	Progress       float64    `json:"progress"`
	FilesProcessed int64      `json:"files_processed"`
	BytesProcessed int64      `json:"bytes_processed"`
	DurationMs     *int64     `json:"duration_ms"`
	VersionID      *string    `json:"version_id"`
	ErrorMessage   *string    `json:"error_message"`
	CheckpointData map[string]interface{} `json:"checkpoint_data"`
	CompletedAt    *time.Time `json:"completed_at"`
}

func (s *Server) agentCompatTaskResult(c *gin.Context) {
	var req compatTaskResultRequest
	if !bindAndValidate(c, &req) {
		return
	}

	execID, err := uuid.Parse(req.ExecutionID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid execution_id"})
		return
	}

	var versionID *uuid.UUID
	if req.VersionID != nil {
		vid, err := uuid.Parse(*req.VersionID)
		if err == nil {
			versionID = &vid
		}
	}

	var completedAt *time.Time
	if req.State == "success" || req.State == "failed" {
		if req.CompletedAt != nil {
			completedAt = req.CompletedAt
		} else {
			now := time.Now().UTC()
			completedAt = &now
		}
	}

	_, err = s.pool.Exec(c.Request.Context(), `
		UPDATE compat_executions
		SET state = $1, progress = $2, files_processed = $3, bytes_processed = $4,
		    duration_ms = $5, version_id = $6, error_message = $7,
		    checkpoint_data = $8, completed_at = COALESCE($9, completed_at)
		WHERE execution_id = $10
	`, req.State, req.Progress, req.FilesProcessed, req.BytesProcessed,
		req.DurationMs, versionID, req.ErrorMessage, req.CheckpointData,
		completedAt, execID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update execution failed"})
		return
	}

	var jobID uuid.UUID
	s.pool.QueryRow(c.Request.Context(), "SELECT job_id FROM compat_executions WHERE execution_id = $1", execID).Scan(&jobID)

	auditAction := "compat_backup_completed"
	auditResult := "success"
	if req.State == "failed" {
		auditAction = "compat_backup_failed"
		auditResult = "failed"
	} else if req.State == "paused" {
		auditAction = "compat_backup_paused"
		auditResult = "paused"
	}

	s.auditLogger.Record(c.Request.Context(), audit.Entry{
		ActorID:    req.AgentID,
		ActorType:  audit.ActorTypeSystem,
		Action:     auditAction,
		TargetType: "compat_execution",
		TargetID:   execID.String(),
		Result:     auditResult,
		TraceID:    traceID(c),
		Detail: map[string]interface{}{
			"job_id":          jobID.String(),
			"files_processed": req.FilesProcessed,
			"bytes_processed": req.BytesProcessed,
		},
	})

	c.JSON(http.StatusOK, gin.H{"accepted": true, "execution_id": execID})
}

type dualCheckResultRequest struct {
	ExecutionID     string                 `json:"execution_id" binding:"required"`
	AgentID         string                 `json:"agent_id"`
	Conclusion      string                 `json:"conclusion" binding:"required"`
	InconsistentFiles []map[string]interface{} `json:"inconsistent_files"`
	TotalFiles      int64                  `json:"total_files"`
	CheckedFiles    int64                  `json:"checked_files"`
	DurationMs      *int64                 `json:"duration_ms"`
}

func (s *Server) agentDualCheckResult(c *gin.Context) {
	var req dualCheckResultRequest
	if !bindAndValidate(c, &req) {
		return
	}

	execID, err := uuid.Parse(req.ExecutionID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid execution_id"})
		return
	}

	state := "success"
	if req.Conclusion == "inconsistent" {
		state = "failed"
	}

	now := time.Now().UTC()
	_, err = s.pool.Exec(c.Request.Context(), `
		UPDATE compat_executions
		SET state = $1, progress = 1.0, files_processed = $2, bytes_processed = $3,
		    duration_ms = $4, checkpoint_data = $5, completed_at = $6
		WHERE execution_id = $7
	`, state, req.CheckedFiles, 0, req.DurationMs,
		map[string]interface{}{
			"conclusion":        req.Conclusion,
			"inconsistent_files": req.InconsistentFiles,
			"total_files":       req.TotalFiles,
		}, &now, execID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update dual check result failed"})
		return
	}

	auditResult := "consistent"
	if req.Conclusion == "inconsistent" {
		auditResult = "inconsistent"
	}

	s.auditLogger.Record(c.Request.Context(), audit.Entry{
		ActorID:    req.AgentID,
		ActorType:  audit.ActorTypeSystem,
		Action:     "dual_repo_check_completed",
		TargetType: "compat_execution",
		TargetID:   execID.String(),
		Result:     auditResult,
		TraceID:    traceID(c),
		Detail: map[string]interface{}{
			"total_files":     req.TotalFiles,
			"checked_files":   req.CheckedFiles,
			"inconsistent_count": len(req.InconsistentFiles),
		},
	})

	c.JSON(http.StatusOK, gin.H{"accepted": true, "execution_id": execID, "conclusion": req.Conclusion})
}