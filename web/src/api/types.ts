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