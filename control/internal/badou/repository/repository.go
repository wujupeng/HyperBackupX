package repository

import (
	"context"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"hbx-control/internal/badou/model"
)

type BadouRepo struct {
	pool *pgxpool.Pool
}

func NewBadouRepo(pool *pgxpool.Pool) *BadouRepo {
	return &BadouRepo{pool: pool}
}

var errNoPool = fmt.Errorf("database connection not available")

func (r *BadouRepo) ListRepositories(ctx context.Context) ([]model.Repository, error) {
	if r.pool == nil {
		return nil, errNoPool
	}
	rows, err := r.pool.Query(ctx, `
		SELECT repo_id, name, description, node_address, node_port,
		       tls_cert_path, tls_key_path, tls_ca_path,
		       jwt_subject, jwt_secret_ref, immutable_retention_days,
		       status, created_at, updated_at
		FROM badou_repositories ORDER BY created_at DESC
	`)
	if err != nil {
		return nil, fmt.Errorf("query badou repos: %w", err)
	}
	defer rows.Close()

	var repos []model.Repository
	for rows.Next() {
		var repo model.Repository
		if err := rows.Scan(
			&repo.ID, &repo.Name, &repo.Description, &repo.NodeAddress, &repo.NodePort,
			&repo.TLSCertPath, &repo.TLSKeyPath, &repo.TLSCAPath,
			&repo.JWTSubject, &repo.JWTSecretRef, &repo.ImmutableRetentionDays,
			&repo.Status, &repo.CreatedAt, &repo.UpdatedAt,
		); err != nil {
			return nil, fmt.Errorf("scan badou repo: %w", err)
		}
		repos = append(repos, repo)
	}
	return repos, nil
}

func (r *BadouRepo) GetRepository(ctx context.Context, id uuid.UUID) (*model.Repository, error) {
	if r.pool == nil {
		return nil, errNoPool
	}
	var repo model.Repository
	err := r.pool.QueryRow(ctx, `
		SELECT repo_id, name, description, node_address, node_port,
		       tls_cert_path, tls_key_path, tls_ca_path,
		       jwt_subject, jwt_secret_ref, immutable_retention_days,
		       status, created_at, updated_at
		FROM badou_repositories WHERE repo_id = $1
	`, id).Scan(
		&repo.ID, &repo.Name, &repo.Description, &repo.NodeAddress, &repo.NodePort,
		&repo.TLSCertPath, &repo.TLSKeyPath, &repo.TLSCAPath,
		&repo.JWTSubject, &repo.JWTSecretRef, &repo.ImmutableRetentionDays,
		&repo.Status, &repo.CreatedAt, &repo.UpdatedAt,
	)
	if err != nil {
		return nil, fmt.Errorf("get badou repo: %w", err)
	}
	return &repo, nil
}

func (r *BadouRepo) CreateRepository(ctx context.Context, req model.CreateRepositoryRequest) (*model.Repository, error) {
	if r.pool == nil {
		return nil, errNoPool
	}
	port := req.NodePort
	if port == 0 {
		port = 50051
	}
	var repo model.Repository
	err := r.pool.QueryRow(ctx, `
		INSERT INTO badou_repositories (name, description, node_address, node_port,
		                                tls_cert_path, tls_key_path, tls_ca_path,
		                                jwt_subject, jwt_secret_ref, immutable_retention_days)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
		RETURNING repo_id, name, description, node_address, node_port,
		          tls_cert_path, tls_key_path, tls_ca_path,
		          jwt_subject, jwt_secret_ref, immutable_retention_days,
		          status, created_at, updated_at
	`, req.Name, req.Description, req.NodeAddress, port,
		req.TLSCertPath, req.TLSKeyPath, req.TLSCAPath,
		req.JWTSubject, req.JWTSecretRef, req.ImmutableRetentionDays,
	).Scan(
		&repo.ID, &repo.Name, &repo.Description, &repo.NodeAddress, &repo.NodePort,
		&repo.TLSCertPath, &repo.TLSKeyPath, &repo.TLSCAPath,
		&repo.JWTSubject, &repo.JWTSecretRef, &repo.ImmutableRetentionDays,
		&repo.Status, &repo.CreatedAt, &repo.UpdatedAt,
	)
	if err != nil {
		return nil, fmt.Errorf("create badou repo: %w", err)
	}
	return &repo, nil
}

func (r *BadouRepo) UpdateRepository(ctx context.Context, id uuid.UUID, req model.UpdateRepositoryRequest) error {
	if r.pool == nil {
		return errNoPool
	}
	_, err := r.pool.Exec(ctx, `
		UPDATE badou_repositories SET
			name = COALESCE($2, name),
			description = COALESCE($3, description),
			node_address = COALESCE($4, node_address),
			node_port = COALESCE($5, node_port),
			status = COALESCE($6, status),
			updated_at = NOW()
		WHERE repo_id = $1
	`, id, req.Name, req.Description, req.NodeAddress, req.NodePort, req.Status)
	if err != nil {
		return fmt.Errorf("update badou repo: %w", err)
	}
	return nil
}

func (r *BadouRepo) DeleteRepository(ctx context.Context, id uuid.UUID) error {
	if r.pool == nil {
		return errNoPool
	}
	_, err := r.pool.Exec(ctx, `DELETE FROM badou_repositories WHERE repo_id = $1`, id)
	if err != nil {
		return fmt.Errorf("delete badou repo: %w", err)
	}
	return nil
}

func (r *BadouRepo) SetImmutableRetention(ctx context.Context, id uuid.UUID, days int) error {
	if r.pool == nil {
		return errNoPool
	}
	_, err := r.pool.Exec(ctx, `
		UPDATE badou_repositories SET immutable_retention_days = $2, updated_at = NOW()
		WHERE repo_id = $1
	`, id, days)
	if err != nil {
		return fmt.Errorf("set immutable retention: %w", err)
	}
	return nil
}

func (r *BadouRepo) ListNodes(ctx context.Context) ([]model.Node, error) {
	if r.pool == nil {
		return nil, errNoPool
	}
	rows, err := r.pool.Query(ctx, `
		SELECT node_id, node_address, node_port, node_role, status,
		       disk_capacity_bytes, disk_used_bytes, joined_at, last_heartbeat_at
		FROM badou_nodes ORDER BY joined_at DESC
	`)
	if err != nil {
		return nil, fmt.Errorf("query badou nodes: %w", err)
	}
	defer rows.Close()

	var nodes []model.Node
	for rows.Next() {
		var node model.Node
		if err := rows.Scan(
			&node.ID, &node.Address, &node.Port, &node.Role, &node.Status,
			&node.DiskCapacityBytes, &node.DiskUsedBytes, &node.JoinedAt, &node.LastHeartbeatAt,
		); err != nil {
			return nil, fmt.Errorf("scan badou node: %w", err)
		}
		nodes = append(nodes, node)
	}
	return nodes, nil
}

func (r *BadouRepo) AddNode(ctx context.Context, req model.AddNodeRequest) (*model.Node, error) {
	if r.pool == nil {
		return nil, errNoPool
	}
	port := req.Port
	if port == 0 {
		port = 50051
	}
	role := req.Role
	if role == "" {
		role = "follower"
	}
	var node model.Node
	err := r.pool.QueryRow(ctx, `
		INSERT INTO badou_nodes (node_address, node_port, node_role, disk_capacity_bytes)
		VALUES ($1, $2, $3, $4)
		RETURNING node_id, node_address, node_port, node_role, status,
		          disk_capacity_bytes, disk_used_bytes, joined_at, last_heartbeat_at
	`, req.Address, port, role, req.DiskCapacityBytes).Scan(
		&node.ID, &node.Address, &node.Port, &node.Role, &node.Status,
		&node.DiskCapacityBytes, &node.DiskUsedBytes, &node.JoinedAt, &node.LastHeartbeatAt,
	)
	if err != nil {
		return nil, fmt.Errorf("add badou node: %w", err)
	}
	return &node, nil
}

func (r *BadouRepo) RemoveNode(ctx context.Context, id uuid.UUID) error {
	if r.pool == nil {
		return errNoPool
	}
	_, err := r.pool.Exec(ctx, `DELETE FROM badou_nodes WHERE node_id = $1`, id)
	if err != nil {
		return fmt.Errorf("remove badou node: %w", err)
	}
	return nil
}

func (r *BadouRepo) UpdateNodeHeartbeat(ctx context.Context, id uuid.UUID) error {
	if r.pool == nil {
		return errNoPool
	}
	_, err := r.pool.Exec(ctx, `
		UPDATE badou_nodes SET last_heartbeat_at = $2 WHERE node_id = $1
	`, id, time.Now().UTC())
	if err != nil {
		return fmt.Errorf("update node heartbeat: %w", err)
	}
	return nil
}

func (r *BadouRepo) CreateGCReport(ctx context.Context, repoID uuid.UUID, triggeredBy string) (uuid.UUID, error) {
	if r.pool == nil {
		return uuid.Nil, errNoPool
	}
	var id uuid.UUID
	err := r.pool.QueryRow(ctx, `
		INSERT INTO badou_gc_reports (repo_id, triggered_by, status)
		VALUES ($1, $2, 'running')
		RETURNING report_id
	`, repoID, triggeredBy).Scan(&id)
	if err != nil {
		return uuid.Nil, fmt.Errorf("create gc report: %w", err)
	}
	return id, nil
}

func (r *BadouRepo) UpdateGCReport(ctx context.Context, reportID uuid.UUID, scanned, deleted, freed, durationMs int64, status string) error {
	if r.pool == nil {
		return errNoPool
	}
	_, err := r.pool.Exec(ctx, `
		UPDATE badou_gc_reports SET
			chunks_scanned = $2, chunks_deleted = $3, bytes_freed = $4,
			duration_ms = $5, status = $6, completed_at = NOW()
		WHERE report_id = $1
	`, reportID, scanned, deleted, freed, durationMs, status)
	if err != nil {
		return fmt.Errorf("update gc report: %w", err)
	}
	return nil
}

func (r *BadouRepo) GetGCReport(ctx context.Context, repoID uuid.UUID) (*model.GCReport, error) {
	if r.pool == nil {
		return nil, errNoPool
	}
	var report model.GCReport
	err := r.pool.QueryRow(ctx, `
		SELECT report_id, repo_id, triggered_by, chunks_scanned, chunks_deleted,
		       bytes_freed, duration_ms, status, started_at, completed_at
		FROM badou_gc_reports
		WHERE repo_id = $1 AND status IN ('completed', 'failed')
		ORDER BY completed_at DESC LIMIT 1
	`, repoID).Scan(
		&report.ReportID, &report.RepoID, &report.TriggeredBy,
		&report.ChunksScanned, &report.ChunksDeleted, &report.BytesFreed,
		&report.DurationMs, &report.Status, &report.StartedAt, &report.CompletedAt,
	)
	if err != nil {
		return nil, fmt.Errorf("get gc report: %w", err)
	}
	return &report, nil
}