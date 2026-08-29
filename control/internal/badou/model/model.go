package model

import (
	"time"

	"github.com/google/uuid"
)

type Repository struct {
	ID                   uuid.UUID `json:"repo_id"`
	Name                 string    `json:"name"`
	Description          string    `json:"description"`
	NodeAddress          string    `json:"node_address"`
	NodePort             int       `json:"node_port"`
	TLSCertPath          string    `json:"tls_cert_path"`
	TLSKeyPath           string    `json:"tls_key_path"`
	TLSCAPath            string    `json:"tls_ca_path"`
	JWTSubject           string    `json:"jwt_subject"`
	JWTSecretRef         string    `json:"jwt_secret_ref"`
	ImmutableRetentionDays int     `json:"immutable_retention_days"`
	Status               string    `json:"status"`
	CreatedAt            time.Time `json:"created_at"`
	UpdatedAt            time.Time `json:"updated_at"`
}

type CreateRepositoryRequest struct {
	Name                 string `json:"name" binding:"required"`
	Description          string `json:"description"`
	NodeAddress          string `json:"node_address" binding:"required"`
	NodePort             int    `json:"node_port"`
	TLSCertPath          string `json:"tls_cert_path"`
	TLSKeyPath           string `json:"tls_key_path"`
	TLSCAPath            string `json:"tls_ca_path"`
	JWTSubject           string `json:"jwt_subject"`
	JWTSecretRef         string `json:"jwt_secret_ref"`
	ImmutableRetentionDays int   `json:"immutable_retention_days"`
}

type UpdateRepositoryRequest struct {
	Name        *string `json:"name"`
	Description *string `json:"description"`
	NodeAddress *string `json:"node_address"`
	NodePort    *int    `json:"node_port"`
	Status      *string `json:"status"`
}

type SetImmutableRequest struct {
	RetentionDays int `json:"retention_days" binding:"required"`
}

type Node struct {
	ID                uuid.UUID `json:"node_id"`
	Address           string    `json:"node_address"`
	Port              int       `json:"node_port"`
	Role              string    `json:"node_role"`
	Status            string    `json:"status"`
	DiskCapacityBytes int64     `json:"disk_capacity_bytes"`
	DiskUsedBytes     int64     `json:"disk_used_bytes"`
	JoinedAt          time.Time `json:"joined_at"`
	LastHeartbeatAt   *time.Time `json:"last_heartbeat_at"`
}

type AddNodeRequest struct {
	Address           string `json:"node_address" binding:"required"`
	Port              int    `json:"node_port"`
	Role              string `json:"node_role"`
	DiskCapacityBytes int64  `json:"disk_capacity_bytes"`
}

type ClusterHealth struct {
	Status       string         `json:"status"`
	TotalNodes   int            `json:"total_nodes"`
	OnlineNodes  int            `json:"online_nodes"`
	LeaderID     string         `json:"leader_id"`
	Nodes        []NodeHealth   `json:"nodes"`
}

type NodeHealth struct {
	NodeID  string `json:"node_id"`
	Address string `json:"address"`
	Status  string `json:"status"`
	Healthy bool   `json:"healthy"`
}

type CapacityRequest struct {
	NodeID          string `json:"node_id" binding:"required"`
	AdditionalBytes int64  `json:"additional_bytes" binding:"required"`
}

type Version struct {
	VersionID  string    `json:"version_id"`
	CreatedAt  time.Time `json:"created_at"`
	Size       int64     `json:"size"`
	ChunkCount int64     `json:"chunk_count"`
	Status     string    `json:"status"`
}

type GCReport struct {
	ReportID      uuid.UUID  `json:"report_id"`
	RepoID        uuid.UUID  `json:"repo_id"`
	TriggeredBy   string     `json:"triggered_by"`
	ChunksScanned int64      `json:"chunks_scanned"`
	ChunksDeleted int64      `json:"chunks_deleted"`
	BytesFreed    int64      `json:"bytes_freed"`
	DurationMs    int64      `json:"duration_ms"`
	Status        string     `json:"status"`
	StartedAt     time.Time  `json:"started_at"`
	CompletedAt   *time.Time `json:"completed_at"`
}

type VerifyRequest struct {
	Level string `json:"level"`
}

type VerifyResult struct {
	RepoID   string `json:"repo_id"`
	Level    string `json:"level"`
	Passed   bool   `json:"passed"`
	Errors   int    `json:"errors"`
	Warnings int    `json:"warnings"`
}