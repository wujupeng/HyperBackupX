package api

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/redis/go-redis/v9"
	"golang.org/x/crypto/bcrypt"

	"hbx-control/internal/audit"
	"hbx-control/internal/auth"
	"hbx-control/internal/compatimport"
	"hbx-control/internal/hbx"
	"hbx-control/internal/rbac"
)

type Server struct {
	pool           *pgxpool.Pool
	redis          *redis.Client
	jwtMgr         *auth.JWTManager
	auditLogger    *audit.Logger
	ca             *hbx.CA
	compatImporter *compatimport.DuplicatiConfigImporter
}

func NewServer(pool *pgxpool.Pool, redis *redis.Client, jwtMgr *auth.JWTManager, auditLogger *audit.Logger) *Server {
	ca, err := hbx.NewCA()
	if err != nil {
		panic("failed to initialize CA: " + err.Error())
	}
	return &Server{
		pool:           pool,
		redis:          redis,
		jwtMgr:         jwtMgr,
		auditLogger:    auditLogger,
		ca:             ca,
		compatImporter: compatimport.NewImporter(compatimport.UnsupportedSkip),
	}
}

func (s *Server) RegisterRoutes(r *gin.Engine) {
	r.Use(CORSMiddleware())
	r.Use(TraceIDMiddleware())
	r.Use(RequestLogMiddleware())
	r.Use(RateLimitMiddleware(100, 200))

	r.GET("/healthz", s.healthz)

	v1 := r.Group("/api/v1")
	{
		v1.GET("/metrics", s.prometheusMetrics)

		authGroup := v1.Group("/auth")
		{
			authGroup.POST("/login", s.login)
			authGroup.POST("/logout", s.JWTAuth(), s.logout)
		}

		authed := v1.Group("")
		authed.Use(s.JWTAuth())
		{
			authed.GET("/devices", s.RBAC(rbac.PermDevicesRead), s.listDevices)
			authed.POST("/devices", s.RBAC(rbac.PermDevicesWrite), s.createDevice)
			authed.DELETE("/devices/:id", s.RBAC(rbac.PermDevicesWrite), s.deleteDevice)
			authed.GET("/devices/:id/policies", s.RBAC(rbac.PermDevicesRead), s.getDevicePolicies)
			authed.PUT("/devices/:id/policies", s.RBAC(rbac.PermDevicesWrite), s.bindDevicePolicies)

			authed.GET("/policies", s.RBAC(rbac.PermPoliciesAll), s.listPolicies)
			authed.POST("/policies", s.RBAC(rbac.PermPoliciesAll), s.createPolicy)
			authed.PUT("/policies/:id", s.RBAC(rbac.PermPoliciesAll), s.updatePolicy)
			authed.DELETE("/policies/:id", s.RBAC(rbac.PermPoliciesAll), s.deletePolicy)
			authed.GET("/policies/:id/versions", s.RBAC(rbac.PermPoliciesAll), s.listPolicyVersions)
			authed.POST("/policies/:id/rollback", s.RBAC(rbac.PermPoliciesAll), s.rollbackPolicy)

			authed.GET("/repositories", s.RBAC(rbac.PermDevicesRead), s.listRepositories)
			authed.POST("/repositories", s.RBAC(rbac.PermDevicesWrite), s.createRepository)
			authed.PUT("/repositories/:id", s.RBAC(rbac.PermDevicesWrite), s.updateRepository)
			authed.DELETE("/repositories/:id", s.RBAC(rbac.PermDevicesWrite), s.deleteRepository)
			authed.POST("/repositories/:id/verify", s.RBAC(rbac.PermDevicesWrite), s.verifyRepository)

			authed.GET("/jobs", s.RBAC(rbac.PermVersionsRead), s.listJobs)
			authed.POST("/jobs", s.RBAC(rbac.PermJobsAll), s.createJob)
			authed.PUT("/jobs/:id", s.RBAC(rbac.PermJobsAll), s.updateJob)
			authed.POST("/jobs/:id/trigger", s.RBAC(rbac.PermJobsTrigger), s.triggerJob)

			authed.GET("/versions", s.RBAC(rbac.PermVersionsRead), s.listVersions)
			authed.GET("/versions/:id/files", s.RBAC(rbac.PermVersionsRead), s.listVersionFiles)

			authed.POST("/restores", s.RBAC(rbac.PermRestoresAll), s.createRestore)
			authed.GET("/restores", s.RBAC(rbac.PermRestoresAll), s.listRestores)
			authed.GET("/restores/:id", s.RBAC(rbac.PermRestoresAll), s.getRestore)

			authed.POST("/verify", s.RBAC(rbac.PermVerifyAll), s.triggerVerify)

			authed.GET("/monitoring/dashboard", s.RBAC(rbac.PermMonitorRead), s.dashboard)
			authed.GET("/monitoring/metrics", s.RBAC(rbac.PermMonitorRead), s.metrics)

			authed.GET("/alerts", s.RBAC(rbac.PermAlertsAll), s.listAlerts)
			authed.PUT("/alerts/:id", s.RBAC(rbac.PermAlertsAll), s.acknowledgeAlert)

			authed.GET("/logs", s.RBAC(rbac.PermLogsRead), s.listLogs)

			authed.GET("/audit", s.RBAC(rbac.PermAuditRead), s.listAuditLogs)

			authed.GET("/users", s.RBAC(rbac.PermUsersAll), s.listUsers)
			authed.POST("/users", s.RBAC(rbac.PermUsersAll), s.createUser)
			authed.PUT("/users/:id", s.RBAC(rbac.PermUsersAll), s.updateUser)
			authed.DELETE("/users/:id", s.RBAC(rbac.PermUsersAll), s.deleteUser)

			authed.GET("/roles", s.RBAC(rbac.PermRolesAll), s.listRoles)
			authed.POST("/roles", s.RBAC(rbac.PermRolesAll), s.createRole)
			authed.PUT("/roles/:id", s.RBAC(rbac.PermRolesAll), s.updateRole)

			authed.GET("/organizations", s.RBAC(rbac.PermOrgsAll), s.listOrganizations)
			authed.POST("/organizations", s.RBAC(rbac.PermOrgsAll), s.createOrganization)
			authed.PUT("/organizations/:id", s.RBAC(rbac.PermOrgsAll), s.updateOrganization)

			authed.POST("/upgrade/agents", s.RBAC(rbac.PermUpgradeAll), s.upgradeAgents)

			compatGroup := authed.Group("/compat")
			{
				compatGroup.GET("/repositories", s.RBAC(rbac.PermCompatRead), s.listCompatRepos)
				compatGroup.POST("/repositories", s.RBAC(rbac.PermCompatWrite), s.createCompatRepo)
				compatGroup.PUT("/repositories/:id", s.RBAC(rbac.PermCompatWrite), s.updateCompatRepo)
				compatGroup.DELETE("/repositories/:id", s.RBAC(rbac.PermCompatWrite), s.deleteCompatRepo)
				compatGroup.POST("/repositories/:id/self-check", s.RBAC(rbac.PermCompatCheck), s.selfCheckCompatRepo)

				compatGroup.GET("/jobs", s.RBAC(rbac.PermCompatRead), s.listCompatJobs)
				compatGroup.POST("/jobs", s.RBAC(rbac.PermCompatWrite), s.createCompatJob)
				compatGroup.PUT("/jobs/:id", s.RBAC(rbac.PermCompatWrite), s.updateCompatJob)
				compatGroup.DELETE("/jobs/:id", s.RBAC(rbac.PermCompatWrite), s.deleteCompatJob)
				compatGroup.POST("/jobs/:id/trigger", s.RBAC(rbac.PermCompatTrigger), s.triggerCompatJob)
				compatGroup.POST("/jobs/:id/dual-check", s.RBAC(rbac.PermCompatCheck), s.dualCheckCompatJob)

				compatGroup.GET("/dual-repo-configs", s.RBAC(rbac.PermCompatRead), s.listDualRepoConfigs)
				compatGroup.POST("/dual-repo-configs", s.RBAC(rbac.PermCompatWrite), s.createDualRepoConfig)
				compatGroup.DELETE("/dual-repo-configs/:id", s.RBAC(rbac.PermCompatWrite), s.deleteDualRepoConfig)

				compatGroup.GET("/executions", s.RBAC(rbac.PermCompatRead), s.listCompatExecutions)
				compatGroup.POST("/executions/report", s.RBAC(rbac.PermCompatTrigger), s.reportCompatExecution)

				compatGroup.GET("/metrics", s.RBAC(rbac.PermCompatRead), s.listCompatMetrics)

				compatGroup.POST("/import", s.RBAC(rbac.PermCompatImport), s.importDuplicatiConfig)
				compatGroup.GET("/import/:id", s.RBAC(rbac.PermCompatRead), s.getImportRecord)
				compatGroup.GET("/imports", s.RBAC(rbac.PermCompatRead), s.listImportRecords)

				compatGroup.GET("/matrix", s.RBAC(rbac.PermCompatRead), s.listMatrixEntries)
				compatGroup.POST("/matrix/execute", s.RBAC(rbac.PermCompatTrigger), s.executeMatrix)
				compatGroup.POST("/golden/execute", s.RBAC(rbac.PermCompatTrigger), s.executeGolden)
				compatGroup.GET("/golden/report", s.RBAC(rbac.PermCompatRead), s.getGoldenReport)
				compatGroup.POST("/dual-run", s.RBAC(rbac.PermCompatTrigger), s.triggerDualRun)
				compatGroup.GET("/dual-run/:id", s.RBAC(rbac.PermCompatRead), s.getDualRunResult)
				compatGroup.GET("/reports", s.RBAC(rbac.PermCompatRead), s.listTestReports)

				compatGroup.POST("/fuzz/execute", s.RBAC(rbac.PermCompatTrigger), s.executeFuzz)
				compatGroup.GET("/fuzz/report", s.RBAC(rbac.PermCompatRead), s.getFuzzReport)
				compatGroup.POST("/chaos/execute", s.RBAC(rbac.PermCompatTrigger), s.executeChaos)
				compatGroup.GET("/chaos/report", s.RBAC(rbac.PermCompatRead), s.getChaosReport)

				compatGroup.GET("/acceptance", s.RBAC(rbac.PermCompatRead), s.getAcceptanceReport)
				compatGroup.POST("/acceptance/trigger", s.RBAC(rbac.PermCompatTrigger), s.triggerAcceptance)
				compatGroup.POST("/acceptance/sign", s.RBAC(rbac.PermCompatWrite), s.signAcceptance)
			}

			s.registerBadouRoutes(authed)
		}

		agentGroup := v1.Group("/agent")
		{
			agentGroup.POST("/register", s.agentRegister)
			agentGroup.POST("/heartbeat", s.agentHeartbeat)
			agentGroup.POST("/task-result", s.agentTaskResult)
			agentGroup.POST("/fetch-policy", s.agentFetchPolicy)
			agentGroup.POST("/status", s.agentStatus)
			agentGroup.POST("/log", s.agentLog)
			agentGroup.POST("/sign-csr", s.agentSignCSR)
			agentGroup.GET("/ca-cert", s.agentCACert)
			agentGroup.GET("/compat-commands", s.getCompatPendingCommands)
			agentGroup.POST("/compat-task-result", s.agentCompatTaskResult)
			agentGroup.POST("/dual-check-result", s.agentDualCheckResult)
		}
	}
}

func (s *Server) JWTAuth() gin.HandlerFunc {
	return JWTAuthMiddleware(s.jwtMgr)
}

func (s *Server) RBAC(perm rbac.Permission) gin.HandlerFunc {
	return RBACMiddleware(perm)
}

func (s *Server) healthz(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 2*time.Second)
	defer cancel()

	status := gin.H{"status": "ok"}
	if s.pool != nil {
		if err := s.pool.Ping(ctx); err != nil {
			status["db"] = "unavailable"
		} else {
			status["db"] = "ok"
		}
	}
	if s.redis != nil {
		if err := s.redis.Ping(ctx).Err(); err != nil {
			status["redis"] = "unavailable"
		} else {
			status["redis"] = "ok"
		}
	}
	c.JSON(http.StatusOK, status)
}

func (s *Server) prometheusMetrics(c *gin.Context) {
	c.Header("Content-Type", "text/plain")
	c.String(http.StatusOK, "# HELP hbx_up 1 if up\n# TYPE hbx_up gauge\nhbx_up 1\n")
}

type loginRequest struct {
	Username string `json:"username" binding:"required"`
	Password string `json:"password" binding:"required"`
}

func (s *Server) login(c *gin.Context) {
	var req loginRequest
	if !bindAndValidate(c, &req) {
		return
	}

	ctx := c.Request.Context()
	var userID uuid.UUID
	var displayName, storedHash, authSource string
	var status string

	err := s.pool.QueryRow(ctx, `
		SELECT user_id, display_name, password_hash, auth_source, status
		FROM users WHERE username = $1
	`, req.Username).Scan(&userID, &displayName, &storedHash, &authSource, &status)
	if err != nil {
		c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
		return
	}
	if status != "active" {
		c.JSON(http.StatusForbidden, gin.H{"error": "account disabled"})
		return
	}
	if authSource == "local" {
		if storedHash == "" {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
			return
		}
		if err := bcrypt.CompareHashAndPassword([]byte(storedHash), []byte(req.Password)); err != nil {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
			return
		}
	}

	rows, err := s.pool.Query(ctx, `
		SELECT r.permissions FROM roles r
		JOIN user_roles ur ON r.role_id = ur.role_id
		WHERE ur.user_id = $1
	`, userID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query roles failed"})
		return
	}
	defer rows.Close()

	var permissions []string
	for rows.Next() {
		var permsJSON []byte
		rows.Scan(&permsJSON)
		var perms []string
		if err := json.Unmarshal(permsJSON, &perms); err == nil {
			permissions = append(permissions, perms...)
		}
	}
	if permissions == nil {
		permissions = []string{}
	}

	token, err := s.jwtMgr.Generate(userID, req.Username, permissions)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "token generation failed"})
		return
	}

	s.auditLogger.Record(ctx, audit.Entry{
		ActorID:    userID.String(),
		ActorType:  audit.ActorTypeUser,
		Action:     "login",
		TargetType: "auth",
		TargetID:   userID.String(),
		Result:     "success",
		TraceID:    traceID(c),
	})

	c.JSON(http.StatusOK, gin.H{
		"token":        token,
		"user_id":      userID,
		"username":     req.Username,
		"display_name": displayName,
		"roles":        permissions,
	})
}

func (s *Server) logout(c *gin.Context) {
	claims := getClaims(c)
	if s.redis != nil {
		s.redis.Del(c.Request.Context(), "session:"+claims.UserID)
	}
	c.JSON(http.StatusOK, gin.H{"status": "logged out"})
}

func (s *Server) listDevices(c *gin.Context) {
	ctx := c.Request.Context()
	rows, err := s.pool.Query(ctx, `
		SELECT device_id, hostname, os_type, agent_version, status, last_heartbeat_at, registered_at
		FROM devices ORDER BY registered_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query devices failed"})
		return
	}
	defer rows.Close()

	var devices []gin.H
	for rows.Next() {
		var id uuid.UUID
		var hostname, osType, agentVer, status string
		var lastHB, registeredAt time.Time
		rows.Scan(&id, &hostname, &osType, &agentVer, &status, &lastHB, &registeredAt)
		devices = append(devices, gin.H{
			"device_id":      id,
			"hostname":       hostname,
			"os_type":        osType,
			"agent_version":  agentVer,
			"status":         status,
			"last_heartbeat": lastHB,
			"registered_at":  registeredAt,
		})
	}
	if devices == nil {
		devices = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"devices": devices})
}

type createDeviceRequest struct {
	Hostname string `json:"hostname" binding:"required"`
	OsType   string `json:"os_type" binding:"required"`
}

func (s *Server) createDevice(c *gin.Context) {
	var req createDeviceRequest
	if !bindAndValidate(c, &req) {
		return
	}
	ctx := c.Request.Context()
	var id uuid.UUID
	err := s.pool.QueryRow(ctx, `
		INSERT INTO devices (hostname, os_type, hardware_profile, agent_version, status)
		VALUES ($1, $2, '{}', '0.1.0', 'offline')
		RETURNING device_id
	`, req.Hostname, req.OsType).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create device failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"device_id": id, "hostname": req.Hostname, "os_type": req.OsType})
}

func (s *Server) deleteDevice(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM devices WHERE device_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete device failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) getDevicePolicies(c *gin.Context) {
	id := c.Param("id")

	_ = id
	c.JSON(http.StatusOK, gin.H{"policies": []gin.H{}})
}

func (s *Server) bindDevicePolicies(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "bound"})
}

func (s *Server) listPolicies(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT policy_id, name, version, scope_type, status, updated_at
		FROM policies ORDER BY updated_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query policies failed"})
		return
	}
	defer rows.Close()

	var policies []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name, scopeType, status string
		var version int
		var updatedAt time.Time
		rows.Scan(&id, &name, &version, &scopeType, &status, &updatedAt)
		policies = append(policies, gin.H{
			"policy_id": id, "name": name, "version": version,
			"scope_type": scopeType, "status": status, "updated_at": updatedAt,
		})
	}
	if policies == nil {
		policies = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"policies": policies})
}

func (s *Server) createPolicy(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	name, _ := req["name"].(string)
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO policies (name, template, scope_type, scope_id)
		VALUES ($1, $2, 'device', gen_random_uuid())
		RETURNING policy_id
	`, name, req).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create policy failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"policy_id": id})
}

func (s *Server) updatePolicy(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) deletePolicy(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM policies WHERE policy_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete policy failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) listPolicyVersions(c *gin.Context) {
	id := c.Param("id")
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT version, created_at FROM policy_versions WHERE policy_id = $1 ORDER BY version DESC
	`, id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query versions failed"})
		return
	}
	defer rows.Close()
	var versions []gin.H
	for rows.Next() {
		var v int
		var t time.Time
		rows.Scan(&v, &t)
		versions = append(versions, gin.H{"version": v, "created_at": t})
	}
	if versions == nil {
		versions = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"versions": versions})
}

func (s *Server) rollbackPolicy(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "rolled back"})
}

func (s *Server) listRepositories(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT repository_id, name, backend_type, status, used_capacity, total_capacity
		FROM repositories ORDER BY created_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query repos failed"})
		return
	}
	defer rows.Close()
	var repos []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name, backendType, status string
		var used, total *int64
		rows.Scan(&id, &name, &backendType, &status, &used, &total)
		repos = append(repos, gin.H{
			"repository_id": id, "name": name, "backend_type": backendType,
			"status": status, "used_capacity": used, "total_capacity": total,
		})
	}
	if repos == nil {
		repos = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"repositories": repos})
}

func (s *Server) createRepository(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	name, _ := req["name"].(string)
	backendType, _ := req["backend_type"].(string)
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO repositories (name, backend_type, connection_config)
		VALUES ($1, $2, $3)
		RETURNING repository_id
	`, name, backendType, req).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create repo failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"repository_id": id})
}

func (s *Server) updateRepository(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) deleteRepository(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM repositories WHERE repository_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete repo failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) verifyRepository(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "verified", "reachable": true})
}

func (s *Server) listJobs(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT job_id, device_id, name, status, created_at
		FROM backup_jobs ORDER BY created_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query jobs failed"})
		return
	}
	defer rows.Close()
	var jobs []gin.H
	for rows.Next() {
		var id, deviceID uuid.UUID
		var name, status string
		var createdAt time.Time
		rows.Scan(&id, &deviceID, &name, &status, &createdAt)
		jobs = append(jobs, gin.H{
			"job_id": id, "device_id": deviceID, "name": name,
			"status": status, "created_at": createdAt,
		})
	}
	if jobs == nil {
		jobs = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"jobs": jobs})
}

func (s *Server) createJob(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	name, _ := req["name"].(string)
	deviceIDStr, _ := req["device_id"].(string)
	deviceID, _ := uuid.Parse(deviceIDStr)
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO backup_jobs (device_id, name, source_config, destination_config)
		VALUES ($1, $2, $3, '{}')
		RETURNING job_id
	`, deviceID, name, req).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create job failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"job_id": id})
}

func (s *Server) updateJob(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) triggerJob(c *gin.Context) {
	id := c.Param("id")
	jobUUID, err := uuid.Parse(id)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid job id"})
		return
	}

	var deviceID uuid.UUID
	var name, status string
	var sourceConfig, destConfig []byte
	err = s.pool.QueryRow(c.Request.Context(), `
		SELECT device_id, name, status, source_config::text, destination_config::text
		FROM backup_jobs WHERE job_id = $1
	`, jobUUID).Scan(&deviceID, &name, &status, &sourceConfig, &destConfig)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "job not found"})
		return
	}

	var agentID uuid.UUID
	err = s.pool.QueryRow(c.Request.Context(), `
		SELECT device_id FROM devices WHERE status IN ('online', 'idle')
		ORDER BY last_heartbeat_at DESC LIMIT 1
	`).Scan(&agentID)
	if err != nil {
		c.JSON(http.StatusServiceUnavailable, gin.H{"error": "no available agent"})
		return
	}

	var sourcePath, repoID, badouGrpc, taskType, targetPath string
	taskType = "backup"
	var sourceMap map[string]interface{}
	if json.Unmarshal(sourceConfig, &sourceMap) == nil {
		if v, ok := sourceMap["source_path"].(string); ok {
			sourcePath = v
		}
		if v, ok := sourceMap["repo_id"].(string); ok {
			repoID = v
		}
		if v, ok := sourceMap["backup_type"].(string); ok && v != "" {
			taskType = v
		}
		if v, ok := sourceMap["target_path"].(string); ok {
			targetPath = v
		}
	}
	if repoID == "" {
		repoID = "default"
	}
	badouGrpc = os.Getenv("HBX_BADOU_GRPC_ENDPOINT")
	if badouGrpc == "" {
		badouGrpc = "http://127.0.0.1:9090"
	}

	taskID := uuid.New()
	spec := map[string]interface{}{
		"task_id":             taskID.String(),
		"job_id":              id,
		"repo_id":             repoID,
		"task_type":           taskType,
		"source_path":         sourcePath,
		"target_path":         targetPath,
		"badou_grpc_endpoint": badouGrpc,
	}
	specJSON, _ := json.Marshal(spec)

	_, err = s.pool.Exec(c.Request.Context(), `
		INSERT INTO pending_tasks (task_id, agent_id, job_id, repo_id, task_type, source_path, badou_grpc_endpoint, spec_json, status)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending')
	`, taskID, agentID, id, repoID, taskType, sourcePath, badouGrpc, specJSON)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "queue task failed: " + err.Error()})
		return
	}

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "job_trigger", "backup_job", id, "triggered", traceID(c)))

	c.JSON(http.StatusAccepted, gin.H{"job_id": id, "task_id": taskID, "agent_id": agentID, "status": "triggered"})
}

func (s *Server) listVersions(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT version_id, job_id, version_number, timestamp, backup_type, status, file_count, total_size, stored_size
		FROM backup_versions ORDER BY timestamp DESC LIMIT 100
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query versions failed"})
		return
	}
	defer rows.Close()
	var versions []gin.H
	for rows.Next() {
		var id, jobID uuid.UUID
		var verNum int64
		var ts time.Time
		var bType, status string
		var fileCount, totalSize, storedSize int64
		rows.Scan(&id, &jobID, &verNum, &ts, &bType, &status, &fileCount, &totalSize, &storedSize)
		versions = append(versions, gin.H{
			"version_id": id, "job_id": jobID, "version_number": verNum,
			"timestamp": ts, "backup_type": bType, "status": status,
			"file_count": fileCount, "total_size": totalSize, "stored_size": storedSize,
		})
	}
	if versions == nil {
		versions = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"versions": versions})
}

func (s *Server) listVersionFiles(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"files": []gin.H{}})
}

func (s *Server) createRestore(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	verIDStr, _ := req["source_version_id"].(string)
	verID, _ := uuid.Parse(verIDStr)
	mode, _ := req["restore_mode"].(string)
	target, _ := req["target_location"].(string)
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO restore_jobs (source_version_id, file_selection, restore_mode, target_location, status)
		VALUES ($1, $2, $3, $4, 'pending')
		RETURNING restore_id
	`, verID, req, mode, target).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create restore failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"restore_id": id})
}

func (s *Server) listRestores(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT restore_id, source_version_id, status, started_at, completed_at
		FROM restore_jobs ORDER BY started_at DESC LIMIT 100
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query restores failed"})
		return
	}
	defer rows.Close()
	var restores []gin.H
	for rows.Next() {
		var id, verID uuid.UUID
		var status string
		var started, completed *time.Time
		rows.Scan(&id, &verID, &status, &started, &completed)
		restores = append(restores, gin.H{
			"restore_id": id, "source_version_id": verID,
			"status": status, "started_at": started, "completed_at": completed,
		})
	}
	if restores == nil {
		restores = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"restores": restores})
}

func (s *Server) getRestore(c *gin.Context) {
	id := c.Param("id")
	var restoreID, verID uuid.UUID
	var status string
	err := s.pool.QueryRow(c.Request.Context(), `
		SELECT restore_id, source_version_id, status FROM restore_jobs WHERE restore_id = $1
	`, id).Scan(&restoreID, &verID, &status)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "restore not found"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"restore_id": restoreID, "source_version_id": verID, "status": status})
}

func (s *Server) triggerVerify(c *gin.Context) {
	c.JSON(http.StatusAccepted, gin.H{"status": "verify_triggered"})
}

func (s *Server) dashboard(c *gin.Context) {
	ctx := c.Request.Context()

	var totalDevices, onlineDevices int
	s.pool.QueryRow(ctx, "SELECT COUNT(*) FROM devices").Scan(&totalDevices)
	s.pool.QueryRow(ctx, "SELECT COUNT(*) FROM devices WHERE status = 'online'").Scan(&onlineDevices)

	var totalJobs, activeJobs int
	s.pool.QueryRow(ctx, "SELECT COUNT(*) FROM backup_jobs").Scan(&totalJobs)
	s.pool.QueryRow(ctx, "SELECT COUNT(*) FROM backup_jobs WHERE status = 'active'").Scan(&activeJobs)

	var totalVersions, totalSize int64
	s.pool.QueryRow(ctx, "SELECT COUNT(*), COALESCE(SUM(total_size), 0) FROM backup_versions").Scan(&totalVersions, &totalSize)

	var activeAlerts int
	s.pool.QueryRow(ctx, "SELECT COUNT(*) FROM alerts WHERE acknowledged = FALSE AND suppressed = FALSE").Scan(&activeAlerts)

	c.JSON(http.StatusOK, gin.H{
		"devices":       gin.H{"total": totalDevices, "online": onlineDevices},
		"jobs":          gin.H{"total": totalJobs, "active": activeJobs},
		"versions":      gin.H{"total": totalVersions, "total_size": totalSize},
		"active_alerts": activeAlerts,
		"timestamp":     time.Now().UTC(),
	})
}

func (s *Server) metrics(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"metrics": []gin.H{
			{"name": "backup_success_rate", "value": 0.99},
			{"name": "avg_throughput_mbps", "value": 120.5},
		},
	})
}

func (s *Server) listAlerts(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT alert_id, rule_id, severity, message, triggered_at, acknowledged
		FROM alerts WHERE acknowledged = FALSE ORDER BY triggered_at DESC LIMIT 100
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query alerts failed"})
		return
	}
	defer rows.Close()
	var alerts []gin.H
	for rows.Next() {
		var id uuid.UUID
		var ruleID, severity, message string
		var triggeredAt time.Time
		var acked bool
		rows.Scan(&id, &ruleID, &severity, &message, &triggeredAt, &acked)
		alerts = append(alerts, gin.H{
			"alert_id": id, "rule_id": ruleID, "severity": severity,
			"message": message, "triggered_at": triggeredAt, "acknowledged": acked,
		})
	}
	if alerts == nil {
		alerts = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"alerts": alerts})
}

func (s *Server) acknowledgeAlert(c *gin.Context) {
	id := c.Param("id")
	claims := getClaims(c)
	userID, _ := uuid.Parse(claims.UserID)
	_, err := s.pool.Exec(c.Request.Context(), `
		UPDATE alerts SET acknowledged = TRUE, acknowledged_by = $1, acknowledged_at = NOW()
		WHERE alert_id = $2
	`, userID, id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "acknowledge failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "acknowledged"})
}

func (s *Server) listLogs(c *gin.Context) {
	deviceID := c.Query("device_id")
	level := c.Query("level")
	limit := 100

	query := `SELECT log_id, device_id, timestamp, level, component, message
		FROM agent_logs ORDER BY timestamp DESC LIMIT $1`
	args := []interface{}{limit}

	if deviceID != "" {
		query = `SELECT log_id, device_id, timestamp, level, component, message
			FROM agent_logs WHERE device_id = $1 ORDER BY timestamp DESC LIMIT $2`
		args = []interface{}{deviceID, limit}
	}
	if level != "" && deviceID != "" {
		query = `SELECT log_id, device_id, timestamp, level, component, message
			FROM agent_logs WHERE device_id = $1 AND level = $2 ORDER BY timestamp DESC LIMIT $3`
		args = []interface{}{deviceID, level, limit}
	}

	rows, err := s.pool.Query(c.Request.Context(), query, args...)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"logs": []gin.H{}})
		return
	}
	defer rows.Close()
	var logs []gin.H
	for rows.Next() {
		var logID int64
		var devID uuid.UUID
		var ts time.Time
		var lvl, comp, msg string
		rows.Scan(&logID, &devID, &ts, &lvl, &comp, &msg)
		logs = append(logs, gin.H{
			"log_id": logID, "device_id": devID, "timestamp": ts,
			"level": lvl, "component": comp, "message": msg,
		})
	}
	if logs == nil {
		logs = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"logs": logs})
}

func (s *Server) listAuditLogs(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT log_id, actor_id, action, target_type, target_id, result, timestamp, trace_id
		FROM audit_logs ORDER BY timestamp DESC LIMIT 100
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"audit_logs": []gin.H{}})
		return
	}
	defer rows.Close()
	var logs []gin.H
	for rows.Next() {
		var id uuid.UUID
		var actorID, action, targetType, targetID, result string
		var ts time.Time
		var traceID *string
		rows.Scan(&id, &actorID, &action, &targetType, &targetID, &result, &ts, &traceID)
		logs = append(logs, gin.H{
			"log_id": id, "actor_id": actorID, "action": action,
			"target_type": targetType, "target_id": targetID,
			"result": result, "timestamp": ts, "trace_id": traceID,
		})
	}
	if logs == nil {
		logs = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"audit_logs": logs})
}

func (s *Server) listUsers(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT user_id, username, display_name, email, auth_source, status, created_at
		FROM users ORDER BY created_at DESC
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query users failed"})
		return
	}
	defer rows.Close()
	var users []gin.H
	for rows.Next() {
		var id uuid.UUID
		var username, displayName, email, authSource, status string
		var createdAt time.Time
		rows.Scan(&id, &username, &displayName, &email, &authSource, &status, &createdAt)
		users = append(users, gin.H{
			"user_id": id, "username": username, "display_name": displayName,
			"email": email, "auth_source": authSource, "status": status,
			"created_at": createdAt,
		})
	}
	if users == nil {
		users = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"users": users})
}

type createUserRequest struct {
	Username    string `json:"username" binding:"required"`
	DisplayName string `json:"display_name" binding:"required"`
	Email       string `json:"email" binding:"required"`
	Password    string `json:"password" binding:"required"`
}

func (s *Server) createUser(c *gin.Context) {
	var req createUserRequest
	if !bindAndValidate(c, &req) {
		return
	}
	hash, err := bcrypt.GenerateFromPassword([]byte(req.Password), bcrypt.DefaultCost)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "hash password failed"})
		return
	}
	var id uuid.UUID
	err = s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO users (username, display_name, email, password_hash, auth_source)
		VALUES ($1, $2, $3, $4, 'local')
		RETURNING user_id
	`, req.Username, req.DisplayName, req.Email, string(hash)).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create user failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"user_id": id})
}

func (s *Server) updateUser(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) deleteUser(c *gin.Context) {
	id := c.Param("id")
	_, err := s.pool.Exec(c.Request.Context(), "DELETE FROM users WHERE user_id = $1", id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "delete user failed"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (s *Server) listRoles(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT role_id, name, is_builtin, permissions FROM roles ORDER BY name
	`)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "query roles failed"})
		return
	}
	defer rows.Close()
	var roles []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name string
		var isBuiltin bool
		var permissions json.RawMessage
		rows.Scan(&id, &name, &isBuiltin, &permissions)
		roles = append(roles, gin.H{
			"role_id": id, "name": name, "is_builtin": isBuiltin,
			"permissions": permissions,
		})
	}
	if roles == nil {
		roles = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"roles": roles})
}

func (s *Server) createRole(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	name, _ := req["name"].(string)
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO roles (name, permissions) VALUES ($1, $2)
		RETURNING role_id
	`, name, req["permissions"]).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create role failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"role_id": id})
}

func (s *Server) updateRole(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) listOrganizations(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT organization_id, name, path, created_at FROM organizations ORDER BY path
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"organizations": []gin.H{}})
		return
	}
	defer rows.Close()
	var orgs []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name, path string
		var createdAt time.Time
		rows.Scan(&id, &name, &path, &createdAt)
		orgs = append(orgs, gin.H{
			"organization_id": id, "name": name, "path": path, "created_at": createdAt,
		})
	}
	if orgs == nil {
		orgs = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"organizations": orgs})
}

func (s *Server) createOrganization(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	name, _ := req["name"].(string)
	var id uuid.UUID
	err := s.pool.QueryRow(c.Request.Context(), `
		INSERT INTO organizations (name, path) VALUES ($1, $2)
		RETURNING organization_id
	`, name, "/"+name).Scan(&id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "create org failed"})
		return
	}
	c.JSON(http.StatusCreated, gin.H{"organization_id": id})
}

func (s *Server) updateOrganization(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (s *Server) upgradeAgents(c *gin.Context) {
	var req map[string]interface{}
	if !bindAndValidate(c, &req) {
		return
	}
	c.JSON(http.StatusAccepted, gin.H{"status": "upgrade_scheduled", "detail": req})
}
