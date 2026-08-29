package api

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"

	"hbx-control/internal/audit"
	"hbx-control/internal/auth"
	"hbx-control/internal/compatimport"
)

func (s *Server) listCompatRepos(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT repo_id, name, root_path, storage_backend, format_version, duplicati_semver, status, created_at, updated_at
		FROM compat_repositories ORDER BY created_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query compat repos failed"})
		return
	}
	defer rows.Close()
	var repos []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name, rootPath, storageBackend, duplicatiSemver, status string
		var formatVersion int
		var createdAt, updatedAt time.Time
		rows.Scan(&id, &name, &rootPath, &storageBackend, &formatVersion, &duplicatiSemver, &status, &createdAt, &updatedAt)
		repos = append(repos, gin.H{
			"repo_id": id, "name": name, "root_path": rootPath,
			"storage_backend": storageBackend, "format_version": formatVersion,
			"duplicati_semver": duplicatiSemver, "status": status,
			"created_at": createdAt, "updated_at": updatedAt,
		})
	}
	if repos == nil {
		repos = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"repositories": repos})
}

type createCompatRepoRequest struct {
	Name           string                 `json:"name" binding:"required"`
	RootPath       string                 `json:"root_path" binding:"required"`
	StorageBackend string                 `json:"storage_backend"`
	BackendConfig  map[string]interface{} `json:"backend_config"`
}

func (s *Server) createCompatRepo(c *gin.Context) {
	var req createCompatRepoRequest
	if !bindAndValidate(c, &req) {
		return
	}
	if req.StorageBackend == "" {
		req.StorageBackend = "local"
	}
	if req.BackendConfig == nil {
		req.BackendConfig = map[string]interface{}{}
	}
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO compat_repositories (name, root_path, storage_backend, backend_config)
		VALUES ($1, $2, $3, $4)
		RETURNING repo_id
	`, req.Name, req.RootPath, req.StorageBackend, req.BackendConfig).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create compat repo failed"})
		return
	}
	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "compat_repo_create", "compat_repository", id.String(), "success", traceID(c)))
	c.JSON(http.StatusCreated, gin.H{"repo_id": id})
}

func (s *Server) updateCompatRepo(c *gin.Context) {
	id := c.Param("id")
	var req createCompatRepoRequest
	if !bindAndValidate(c, &req) {
		return
	}
	_, err := s.pool.Exec(c.Request.Context(), `
		UPDATE compat_repositories SET name = $1, root_path = $2, storage_backend = $3, backend_config = $4, updated_at = NOW()
		WHERE repo_id = $5
	`, req.Name, req.RootPath, req.StorageBackend, req.BackendConfig, id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update compat repo failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) deleteCompatRepo(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM compat_repositories WHERE repo_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete compat repo failed"})
		return
	}
	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "compat_repo_delete", "compat_repository", id, "success", traceID(c)))
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) selfCheckCompatRepo(c *gin.Context) {
	id := c.Param("id")
	var exists bool
	err := s.pool.QueryRow(c.Request.Context(), "SELECT EXISTS(SELECT 1 FROM compat_repositories WHERE repo_id = $1)", id).Scan(&exists)
	if err != nil || !exists {
		c.JSON(http.StatusNotFound, gin.H{"error": "compat repo not found"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"repo_id": id, "status": "healthy", "issues": []gin.H{}})
}

func (s *Server) listCompatJobs(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT job_id, name, repo_id, backup_type, dual_repo_mode, status, created_at, updated_at
		FROM compat_jobs ORDER BY created_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query compat jobs failed"})
		return
	}
	defer rows.Close()
	var jobs []gin.H
	for rows.Next() {
		var id, repoID uuid.UUID
		var name, backupType, dualRepoMode, status string
		var createdAt, updatedAt time.Time
		rows.Scan(&id, &name, &repoID, &backupType, &dualRepoMode, &status, &createdAt, &updatedAt)
		jobs = append(jobs, gin.H{
			"job_id": id, "name": name, "repo_id": repoID,
			"backup_type": backupType, "dual_repo_mode": dualRepoMode,
			"status": status, "created_at": createdAt, "updated_at": updatedAt,
		})
	}
	if jobs == nil {
		jobs = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"jobs": jobs})
}

type createCompatJobRequest struct {
	Name              string                 `json:"name" binding:"required"`
	RepoID            string                 `json:"repo_id" binding:"required"`
	BackupType        string                 `json:"backup_type"`
	SourceConfig      map[string]interface{} `json:"source_config"`
	ScheduleConfig    map[string]interface{} `json:"schedule_config"`
	RetentionConfig   map[string]interface{} `json:"retention_config"`
	EncryptionConfig  map[string]interface{} `json:"encryption_config"`
	CompressionConfig map[string]interface{} `json:"compression_config"`
	DualRepoMode      string                 `json:"dual_repo_mode"`
	DualRepoConfigID  string                 `json:"dual_repo_config_id"`
}

func (s *Server) createCompatJob(c *gin.Context) {
	var req createCompatJobRequest
	if !bindAndValidate(c, &req) {
		return
	}
	repoID, err := uuid.Parse(req.RepoID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repo_id"})
		return
	}
	if req.BackupType == "" {
		req.BackupType = "full"
	}
	if req.DualRepoMode == "" {
		req.DualRepoMode = "compatible_only"
	}
	if req.SourceConfig == nil {
		req.SourceConfig = map[string]interface{}{}
	}
	if req.ScheduleConfig == nil {
		req.ScheduleConfig = map[string]interface{}{}
	}
	if req.RetentionConfig == nil {
		req.RetentionConfig = map[string]interface{}{}
	}
	if req.EncryptionConfig == nil {
		req.EncryptionConfig = map[string]interface{}{}
	}
	if req.CompressionConfig == nil {
		req.CompressionConfig = map[string]interface{}{}
	}

	var dualRepoConfigID *uuid.UUID
	if req.DualRepoConfigID != "" {
		parsed, err := uuid.Parse(req.DualRepoConfigID)
		if err == nil {
			dualRepoConfigID = &parsed
		}
	}

	var id uuid.UUID
	err = s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO compat_jobs (name, repo_id, source_config, backup_type, schedule_config, retention_config, encryption_config, compression_config, dual_repo_mode, dual_repo_config_id)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
		RETURNING job_id
	`, req.Name, repoID, req.SourceConfig, req.BackupType, req.ScheduleConfig, req.RetentionConfig, req.EncryptionConfig, req.CompressionConfig, req.DualRepoMode, dualRepoConfigID).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create compat job failed"})
		return
	}
	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "compat_job_create", "compat_job", id.String(), "success", traceID(c)))
	c.JSON(http.StatusCreated, gin.H{"job_id": id})
}

func (s *Server) updateCompatJob(c *gin.Context) {
	id := c.Param("id")
	var req createCompatJobRequest
	if !bindAndValidate(c, &req) {
		return
	}
	_, err := s.pool.Exec(c.Request.Context(), `
		UPDATE compat_jobs SET name = $1, backup_type = $2, source_config = $3, dual_repo_mode = $4, updated_at = NOW()
		WHERE job_id = $5
	`, req.Name, req.BackupType, req.SourceConfig, req.DualRepoMode, id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update compat job failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) deleteCompatJob(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM compat_jobs WHERE job_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete compat job failed"})
		return
	}
	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "compat_job_delete", "compat_job", id, "success", traceID(c)))
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) triggerCompatJob(c *gin.Context) {
	id := c.Param("id")
	var exists bool
	err := s.pool.QueryRow(c.Request.Context(), "SELECT EXISTS(SELECT 1 FROM compat_jobs WHERE job_id = $1)", id).Scan(&exists)
	if err != nil || !exists {
		c.JSON(http.StatusNotFound, gin.H{"error": "compat job not found"})
		return
	}
	var execID uuid.UUID
	err = s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO compat_executions (job_id, state) VALUES ($1, 'pending') RETURNING execution_id
	`, id).Scan(&execID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create execution failed"})
		return
	}
	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "compat_job_trigger", "compat_job", id, "triggered", traceID(c)))
	c.JSON(http.StatusAccepted, gin.H{"job_id": id, "execution_id": execID, "status": "triggered"})
}

func (s *Server) dualCheckCompatJob(c *gin.Context) {
	id := c.Param("id")
	var jobID uuid.UUID
	var dualRepoMode string
	err := s.pool.QueryRow(c.Request.Context(), "SELECT job_id, dual_repo_mode FROM compat_jobs WHERE job_id = $1", id).Scan(&jobID, &dualRepoMode)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "compat job not found"})
		return
	}
	if dualRepoMode != "dual_with_consistency" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "job is not in dual_with_consistency mode"})
		return
	}
	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "dual_repo_check", "compat_job", id, "triggered", traceID(c)))
	c.JSON(http.StatusAccepted, gin.H{"job_id": id, "status": "dual_check_triggered"})
}

func (s *Server) listDualRepoConfigs(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT config_id, name, native_repo_id, compat_repo_id, consistency_mode, auto_repair, alert_on_inconsistency, created_at
		FROM dual_repo_configs ORDER BY created_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query dual repo configs failed"})
		return
	}
	defer rows.Close()
	var configs []gin.H
	for rows.Next() {
		var id, nativeRepoID, compatRepoID uuid.UUID
		var name, consistencyMode string
		var autoRepair, alertOnInconsistency bool
		var createdAt time.Time
		rows.Scan(&id, &name, &nativeRepoID, &compatRepoID, &consistencyMode, &autoRepair, &alertOnInconsistency, &createdAt)
		configs = append(configs, gin.H{
			"config_id": id, "name": name, "native_repo_id": nativeRepoID,
			"compat_repo_id": compatRepoID, "consistency_mode": consistencyMode,
			"auto_repair": autoRepair, "alert_on_inconsistency": alertOnInconsistency,
			"created_at": createdAt,
		})
	}
	if configs == nil {
		configs = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"configs": configs})
}

type createDualRepoConfigRequest struct {
	Name                string `json:"name" binding:"required"`
	NativeRepoID        string `json:"native_repo_id" binding:"required"`
	CompatRepoID        string `json:"compat_repo_id" binding:"required"`
	ConsistencyMode     string `json:"consistency_mode"`
	AutoRepair          bool   `json:"auto_repair"`
	AlertOnInconsistency bool   `json:"alert_on_inconsistency"`
}

func (s *Server) createDualRepoConfig(c *gin.Context) {
	var req createDualRepoConfigRequest
	if !bindAndValidate(c, &req) {
		return
	}
	nativeRepoID, err := uuid.Parse(req.NativeRepoID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid native_repo_id"})
		return
	}
	compatRepoID, err := uuid.Parse(req.CompatRepoID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid compat_repo_id"})
		return
	}
	if req.ConsistencyMode == "" {
		req.ConsistencyMode = "sha256"
	}
	if !req.AlertOnInconsistency {
		req.AlertOnInconsistency = true
	}
	var id uuid.UUID
	err = s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO dual_repo_configs (name, native_repo_id, compat_repo_id, consistency_mode, auto_repair, alert_on_inconsistency)
		VALUES ($1, $2, $3, $4, $5, $6)
		RETURNING config_id
	`, req.Name, nativeRepoID, compatRepoID, req.ConsistencyMode, req.AutoRepair, req.AlertOnInconsistency).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create dual repo config failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"config_id": id})
}

func (s *Server) deleteDualRepoConfig(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM dual_repo_configs WHERE config_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete dual repo config failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) listCompatExecutions(c *gin.Context) {
	jobID := c.Query("job_id")
	query := `SELECT execution_id, job_id, state, progress, files_processed, bytes_processed, error_message, started_at, completed_at
		FROM compat_executions ORDER BY started_at DESC LIMIT 100`
	args := []interface{}{}
	if jobID != "" {
		query = `SELECT execution_id, job_id, state, progress, files_processed, bytes_processed, error_message, started_at, completed_at
			FROM compat_executions WHERE job_id = $1 ORDER BY started_at DESC LIMIT 100`
		args = []interface{}{jobID}
	}
	rows, err := s.pool.Query(c.Request.Context(), query, args...)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query compat executions failed"})
		return
	}
	defer rows.Close()
	var executions []gin.H
	for rows.Next() {
		var id, jID uuid.UUID
		var state string
		var progress float64
		var filesProcessed, bytesProcessed int64
		var errMsg *string
		var startedAt time.Time
		var completedAt *time.Time
		rows.Scan(&id, &jID, &state, &progress, &filesProcessed, &bytesProcessed, &errMsg, &startedAt, &completedAt)
		executions = append(executions, gin.H{
			"execution_id": id, "job_id": jID, "state": state,
			"progress": progress, "files_processed": filesProcessed,
			"bytes_processed": bytesProcessed, "error_message": errMsg,
			"started_at": startedAt, "completed_at": completedAt,
		})
	}
	if executions == nil {
		executions = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"executions": executions})
}

func (s *Server) reportCompatExecution(c *gin.Context) {
	var req struct {
		ExecutionID    string  `json:"execution_id" binding:"required"`
		State          string  `json:"state" binding:"required"`
		Progress       float64 `json:"progress"`
		FilesProcessed int64   `json:"files_processed"`
		BytesProcessed int64   `json:"bytes_processed"`
		ErrorMessage   string  `json:"error_message"`
	}
	if !bindAndValidate(c, &req) {
		return
	}
	execID, err := uuid.Parse(req.ExecutionID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid execution_id"})
		return
	}
	var completedAt *time.Time
	if req.State == "success" || req.State == "failed" {
		now := time.Now().UTC()
		completedAt = &now
	}
	var errMsg *string
	if req.ErrorMessage != "" {
		errMsg = &req.ErrorMessage
	}
	_, err = s.pool.Exec(c.Request.Context(), `
		UPDATE compat_executions SET state = $1, progress = $2, files_processed = $3, bytes_processed = $4, error_message = $5, completed_at = COALESCE($6, completed_at)
		WHERE execution_id = $7
	`, req.State, req.Progress, req.FilesProcessed, req.BytesProcessed, errMsg, completedAt, execID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update execution failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) listCompatMetrics(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT metric_id, metric_name, metric_value, labels, recorded_at
		FROM compat_metrics ORDER BY recorded_at DESC LIMIT 200
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"metrics": []gin.H{}})
		return
	}
	defer rows.Close()
	var metrics []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name string
		var value float64
		var labels []byte
		var recordedAt time.Time
		rows.Scan(&id, &name, &value, &labels, &recordedAt)
		metrics = append(metrics, gin.H{
			"metric_id": id, "name": name, "value": value,
			"labels": labels, "recorded_at": recordedAt,
		})
	}
	if metrics == nil {
		metrics = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"metrics": metrics})
}

func auditEntry(claims *auth.Claims, action, targetType, targetID, result, traceID string) audit.Entry {
	return audit.Entry{
		ActorID:    claims.UserID,
		ActorType:  audit.ActorTypeUser,
		Action:     action,
		TargetType: targetType,
		TargetID:   targetID,
		Result:     result,
		TraceID:    traceID,
	}
}

type importConfigRequest struct {
	Format string `json:"format" binding:"required"`
	Config []byte `json:"config" binding:"required"`
}

func (s *Server) importDuplicatiConfig(c *gin.Context) {
	var req importConfigRequest
	if !bindAndValidate(c, &req) {
		return
	}

	var format compatimport.SourceFormat
	switch req.Format {
	case "json":
		format = compatimport.FormatJSON
	case "sqlite":
		format = compatimport.FormatSQLite
	case "xml":
		format = compatimport.FormatXML
	default:
		c.JSON(http.StatusBadRequest, gin.H{"error": "unsupported format, use json/sqlite/xml"})
		return
	}

	result, err := s.compatImporter.Import(format, req.Config)

	claims := getClaims(c)
	tid := traceID(c)

	if err != nil {
		s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "config_import", "duplicati_config", result.SourceHash, "failed", tid))
		c.JSON(http.StatusBadRequest, gin.H{
			"error":    err.Error(),
			"import_id": result.ImportID,
			"status":   result.Status,
		})
		return
	}

	var resultingJobID *uuid.UUID
	if result.ResultingJobID != nil {
		resultingJobID = result.ResultingJobID
		var jobID uuid.UUID
		insertErr := s.pool.QueryRow(c.Request.Context(), `
			INSERT INTO duplicati_config_imports (source_config_hash, source_format, source_config, resulting_job_id, field_mappings, unsupported_items, import_status)
			VALUES ($1, $2, $3, $4, $5, $6, $7)
			RETURNING import_id
		`, result.SourceHash, req.Format, req.Config, resultingJobID, result.FieldMappings, result.UnsupportedItems, string(result.Status)).Scan(&jobID)
		if insertErr != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "persist import record failed"})
			return
		}
	} else {
		var jobID uuid.UUID
		insertErr := s.pool.QueryRow(c.Request.Context(), `
			INSERT INTO duplicati_config_imports (source_config_hash, source_format, source_config, field_mappings, unsupported_items, import_status)
			VALUES ($1, $2, $3, $4, $5, $6)
			RETURNING import_id
		`, result.SourceHash, req.Format, req.Config, result.FieldMappings, result.UnsupportedItems, string(result.Status)).Scan(&jobID)
		if insertErr != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "persist import record failed"})
			return
		}
	}

	auditResult := "success"
	if result.Status == compatimport.ImportPartial {
		auditResult = "partial"
	} else if result.Status == compatimport.ImportFailed {
		auditResult = "failed"
	}
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "config_import", "duplicati_config", result.SourceHash, auditResult, tid))

	c.JSON(http.StatusOK, gin.H{
		"import_id":         result.ImportID,
		"status":            result.Status,
		"resulting_job_id":  result.ResultingJobID,
		"field_mappings":    result.FieldMappings,
		"unsupported_items": result.UnsupportedItems,
		"idempotent":        result.Idempotent,
	})
}

func (s *Server) getImportRecord(c *gin.Context) {
	id := c.Param("id")
	var importID uuid.UUID
	var sourceHash, sourceFormat, importStatus string
	var sourceConfig []byte
	var fieldMappings []byte
	var unsupportedItems []byte
	var resultingJobID *uuid.UUID
	var importedAt time.Time

	err := s.pool.QueryRow(c.Request.Context(), `
		SELECT import_id, source_config_hash, source_format, source_config, resulting_job_id, field_mappings, unsupported_items, import_status, imported_at
		FROM duplicati_config_imports WHERE import_id = $1
	`, id).Scan(&importID, &sourceHash, &sourceFormat, &sourceConfig, &resultingJobID, &fieldMappings, &unsupportedItems, &importStatus, &importedAt)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "import record not found"})
		return
	}
	c.JSON(http.StatusOK, gin.H{
		"import_id":         importID,
		"source_config_hash": sourceHash,
		"source_format":     sourceFormat,
		"resulting_job_id":  resultingJobID,
		"field_mappings":    fieldMappings,
		"unsupported_items": unsupportedItems,
		"import_status":     importStatus,
		"imported_at":       importedAt,
	})
}

func (s *Server) listImportRecords(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT import_id, source_config_hash, source_format, import_status, imported_at
		FROM duplicati_config_imports ORDER BY imported_at DESC LIMIT 100
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"imports": []gin.H{}})
		return
	}
	defer rows.Close()
	var imports []gin.H
	for rows.Next() {
		var id uuid.UUID
		var hash, format, status string
		var importedAt time.Time
		rows.Scan(&id, &hash, &format, &status, &importedAt)
		imports = append(imports, gin.H{
			"import_id": id, "source_config_hash": hash,
			"source_format": format, "import_status": status,
			"imported_at": importedAt,
		})
	}
	if imports == nil {
		imports = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"imports": imports})
}