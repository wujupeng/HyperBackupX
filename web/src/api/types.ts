export interface User {
  user_id: string;
  username: string;
  display_name: string;
  email: string;
  auth_source: string;
  status: string;
  created_at: string;
}

export interface Device {
  device_id: string;
  hostname: string;
  os_type: string;
  agent_version: string;
  status: 'online' | 'offline' | 'disabled';
  last_heartbeat: string;
  registered_at: string;
}

export interface Policy {
  policy_id: string;
  name: string;
  version: number;
  scope_type: string;
  status: string;
  updated_at: string;
}

export interface Repository {
  repository_id: string;
  name: string;
  backend_type: string;
  status: string;
  used_capacity: number | null;
  total_capacity: number | null;
  connection_config?: Record<string, unknown>;
  created_at?: string;
}

export interface BackupJob {
  job_id: string;
  device_id: string;
  name: string;
  status: string;
  created_at: string;
}

export interface BackupVersion {
  version_id: string;
  job_id: string;
  version_number: number;
  timestamp: string;
  backup_type: string;
  status: string;
  file_count: number;
  total_size: number;
  stored_size: number;
}

export interface RestoreJob {
  restore_id: string;
  source_version_id: string;
  status: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface Alert {
  alert_id: string;
  rule_id: string;
  severity: string;
  message: string;
  triggered_at: string;
  acknowledged: boolean;
}

export interface AuditLog {
  log_id: string;
  actor_id: string;
  action: string;
  target_type: string;
  target_id: string;
  result: string;
  timestamp: string;
  trace_id: string | null;
}

export interface AgentLog {
  log_id: number;
  device_id: string;
  timestamp: string;
  level: string;
  component: string;
  trace_id: string | null;
  message: string;
}

export interface Role {
  role_id: string;
  name: string;
  is_builtin: boolean;
  permissions: string[];
}

export interface Organization {
  organization_id: string;
  name: string;
  path: string;
  created_at: string;
}

export interface DashboardData {
  devices: { total: number; online: number };
  jobs: { total: number; active: number };
  versions: { total: number; total_size: number };
  active_alerts: number;
  timestamp: string;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  user_id: string;
  username: string;
  display_name: string;
  roles: string[];
}

export interface ApiListResponse<T> {
  [key: string]: T[];
}

export type BackendType = 'local' | 'smb' | 'ftp' | 'ftps' | 'sftp' | 'webdav' | 's3' | 'azure_blob' | 'gcs' | 'openstack';

export interface BindPoliciesRequest {
  policy_ids: string[];
}

export interface JobCreateRequest {
  name: string;
  device_id: string;
  backup_config?: Record<string, unknown>;
  source_config?: Record<string, unknown>;
  destination_config?: Record<string, unknown>;
  schedule?: Record<string, unknown>;
  retention?: Record<string, unknown>;
  encryption?: Record<string, unknown>;
}

export interface JobUpdateRequest {
  name?: string;
  device_id?: string;
  backup_config?: Record<string, unknown>;
}

export interface RepositoryCreateRequest {
  name: string;
  backend_type: BackendType;
  connection_config: Record<string, unknown>;
}

export interface RepositoryUpdateRequest {
  name?: string;
  connection_config?: Record<string, unknown>;
}

export interface RepositoryVerifyResponse {
  status: string;
  reachable: boolean;
  message?: string;
}

export interface BadouRepository {
  repo_id: string;
  name: string;
  description: string;
  node_address: string;
  node_port: number;
  tls_cert_path: string;
  tls_key_path: string;
  tls_ca_path: string;
  jwt_subject: string;
  jwt_secret_ref: string;
  immutable_retention_days: number;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface BadouCreateRepoRequest {
  name: string;
  description?: string;
  node_address: string;
  node_port?: number;
  tls_cert_path?: string;
  tls_key_path?: string;
  tls_ca_path?: string;
  jwt_subject?: string;
  jwt_secret_ref?: string;
  immutable_retention_days?: number;
}

export interface BadouVersion {
  version_id: string;
  created_at: string;
  size: number;
  chunk_count: number;
  status: string;
}

export interface BadouGCReport {
  report_id: string;
  repo_id: string;
  triggered_by: string;
  chunks_scanned: number;
  chunks_deleted: number;
  bytes_freed: number;
  duration_ms: number;
  status: string;
  started_at: string;
  completed_at: string | null;
}

export interface BadouVerifyResult {
  repo_id: string;
  level: string;
  passed: boolean;
  errors: number;
  warnings: number;
}

export interface BadouNode {
  node_id: string;
  node_address: string;
  node_port: number;
  node_role: string;
  status: string;
  disk_capacity_bytes: number;
  disk_used_bytes: number;
  joined_at: string;
  last_heartbeat_at: string | null;
}

export interface BadouClusterHealth {
  status: string;
  total_nodes: number;
  online_nodes: number;
  leader_id: string;
  nodes: { node_id: string; address: string; status: string; healthy: boolean }[];
}