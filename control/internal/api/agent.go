package api

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
)

type signCSRRequest struct {
	DeviceID string `json:"device_id" binding:"required"`
	CSRPem   string `json:"csr_pem" binding:"required"`
}

type signCSRResponse struct {
	CertPem string `json:"cert_pem"`
	CaPem   string `json:"ca_pem"`
}

func (s *Server) agentSignCSR(c *gin.Context) {
	var req signCSRRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	certPEM, err := s.ca.SignCSR([]byte(req.CSRPem), req.DeviceID)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "CSR signing failed: " + err.Error()})
		return
	}

	c.JSON(http.StatusOK, signCSRResponse{
		CertPem: string(certPEM),
		CaPem:   string(s.ca.CACertPEM()),
	})
}

func (s *Server) agentCACert(c *gin.Context) {
	c.Data(http.StatusOK, "application/x-pem-file", s.ca.CACertPEM())
}

type registerDeviceRequest struct {
	Hostname           string   `json:"hostname"`
	OsVersion          string   `json:"os_version"`
	AgentVersion       string   `json:"agent_version"`
	Tier               string   `json:"tier"`
	SupportedProtocols []string `json:"supported_protocols"`
	DeviceFingerprint  string   `json:"device_fingerprint"`
}

type registerDeviceResponse struct {
	AgentID               string `json:"agent_id"`
	AssignedGroup         string `json:"assigned_group"`
	MtlsCertPem           string `json:"mtls_cert_pem"`
	MtlsCaPem             string `json:"mtls_ca_pem"`
	HeartbeatIntervalSecs uint32 `json:"heartbeat_interval_secs"`
}

func (s *Server) agentRegister(c *gin.Context) {
	var req registerDeviceRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	agentID := uuid.New().String()

	if s.pool != nil {
		ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
		defer cancel()
		_, err := s.pool.Exec(ctx,
			`INSERT INTO devices (device_id, hostname, os_type, agent_version, hardware_profile, status, registered_at, last_heartbeat_at)
			 VALUES ($1, $2, $3, $4, $5, 'online', NOW(), NOW())`,
			agentID, req.Hostname, req.OsVersion, req.AgentVersion, fmt.Sprintf(`{"tier":"%s"}`, req.Tier))
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to register device: " + err.Error()})
			return
		}
	}

	c.JSON(http.StatusOK, registerDeviceResponse{
		AgentID:               agentID,
		AssignedGroup:         "default",
		MtlsCaPem:             string(s.ca.CACertPEM()),
		HeartbeatIntervalSecs: 30,
	})
}

type heartbeatRequest struct {
	AgentID         string          `json:"agent_id"`
	Timestamp       time.Time       `json:"timestamp"`
	Status          string          `json:"status"`
	Resources       json.RawMessage `json:"resources"`
	ProtocolVersion string          `json:"protocol_version"`
}

type heartbeatResponse struct {
	ServerTime      time.Time `json:"server_time"`
	PendingCommands []string  `json:"pending_commands"`
	ConfigUpdated   bool      `json:"config_updated"`
}

func (s *Server) agentHeartbeat(c *gin.Context) {
	var req heartbeatRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if s.pool != nil {
		ctx, cancel := context.WithTimeout(c.Request.Context(), 3*time.Second)
		defer cancel()
		s.pool.Exec(ctx,
			`UPDATE devices SET last_heartbeat_at = NOW(), status = $2 WHERE device_id = $1`,
			req.AgentID, req.Status)
	}

	var pendingCommands []string
	if s.pool != nil {
		ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
		defer cancel()
		rows, err := s.pool.Query(ctx,
			`UPDATE pending_tasks SET status = 'dispatched', dispatched_at = NOW()
			 WHERE task_id IN (
				 SELECT task_id FROM pending_tasks
				 WHERE (agent_id = $1 OR agent_id IS NULL) AND status = 'pending'
				 ORDER BY created_at LIMIT 5
			 )
			 RETURNING spec_json::text`,
			req.AgentID)
		if err == nil {
			defer rows.Close()
			for rows.Next() {
				var cmd string
				rows.Scan(&cmd)
				pendingCommands = append(pendingCommands, cmd)
			}
		}
	}
	if pendingCommands == nil {
		pendingCommands = []string{}
	}

	c.JSON(http.StatusOK, heartbeatResponse{
		ServerTime:      time.Now().UTC(),
		PendingCommands: pendingCommands,
		ConfigUpdated:   false,
	})
}

type taskResultRequest struct {
	TaskID         string     `json:"task_id"`
	AgentID        string     `json:"agent_id"`
	JobID          string     `json:"job_id"`
	Status         string     `json:"status"`
	StartedAt      time.Time  `json:"started_at"`
	CompletedAt    *time.Time `json:"completed_at"`
	BytesProcessed uint64     `json:"bytes_processed"`
	BytesStored    uint64     `json:"bytes_stored"`
	FileCount      uint32     `json:"file_count"`
	ChunkCount     uint32     `json:"chunk_count"`
	DedupRatio     float64    `json:"dedup_ratio"`
	VersionID      *string    `json:"version_id"`
	ErrorMessage   *string    `json:"error_message"`
	TraceID        string     `json:"trace_id"`
}

func (s *Server) agentTaskResult(c *gin.Context) {
	var req taskResultRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if s.pool != nil {
		ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
		defer cancel()
		s.pool.Exec(ctx,
			`INSERT INTO task_results (id, task_id, agent_id, job_id, status, started_at, completed_at,
			 bytes_processed, bytes_stored, file_count, chunk_count, dedup_ratio, version_id, error_message, trace_id, created_at)
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, NOW())`,
			uuid.New().String(), req.TaskID, req.AgentID, req.JobID, req.Status,
			req.StartedAt, req.CompletedAt, req.BytesProcessed, req.BytesStored,
			req.FileCount, req.ChunkCount, req.DedupRatio, req.VersionID, req.ErrorMessage, req.TraceID)
	}

	c.JSON(http.StatusOK, gin.H{"accepted": true, "message": "task result recorded"})
}

type fetchPolicyRequest struct {
	AgentID              string `json:"agent_id"`
	CurrentPolicyVersion string `json:"current_policy_version"`
}

type fetchPolicyResponse struct {
	PolicyID      string `json:"policy_id"`
	PolicyVersion string `json:"policy_version"`
	PolicyPayload []byte `json:"policy_payload"`
	Unchanged     bool   `json:"unchanged"`
}

func (s *Server) agentFetchPolicy(c *gin.Context) {
	var req fetchPolicyRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, fetchPolicyResponse{
		Unchanged: true,
	})
}

type statusReportRequest struct {
	AgentID   string          `json:"agent_id"`
	Timestamp time.Time       `json:"timestamp"`
	Payload   json.RawMessage `json:"payload"`
}

func (s *Server) agentStatus(c *gin.Context) {
	var req statusReportRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if s.redis != nil {
		ctx, cancel := context.WithTimeout(c.Request.Context(), 3*time.Second)
		defer cancel()
		s.redis.Set(ctx, "agent:status:"+req.AgentID, req.Payload, 5*time.Minute)
	}

	c.JSON(http.StatusOK, gin.H{"accepted": true})
}

type logEntryRequest struct {
	AgentID   string            `json:"agent_id"`
	Timestamp time.Time         `json:"timestamp"`
	Level     string            `json:"level"`
	Message   string            `json:"message"`
	TraceID   string            `json:"trace_id"`
	Fields    map[string]string `json:"fields"`
}

func (s *Server) agentLog(c *gin.Context) {
	var req logEntryRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if s.pool != nil {
		ctx, cancel := context.WithTimeout(c.Request.Context(), 3*time.Second)
		defer cancel()
		s.pool.Exec(ctx,
			`INSERT INTO agent_logs (device_id, timestamp, level, component, message, trace_id, fields)
			 VALUES ($1, $2, $3, 'agent', $4, $5, '{}'::jsonb)`,
			req.AgentID, req.Timestamp, req.Level, req.Message, req.TraceID)
	}

	c.JSON(http.StatusOK, gin.H{"accepted": true})
}
