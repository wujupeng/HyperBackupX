import { get, post, put, del } from './client';

export interface CompatRepository {
  repo_id: string;
  name: string;
  root_path: string;
  storage_backend: string;
  format_version: number;
  duplicati_semver: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface CompatJob {
  job_id: string;
  name: string;
  repo_id: string;
  backup_type: string;
  dual_repo_mode: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface DualRepoConfig {
  config_id: string;
  name: string;
  native_repo_id: string;
  compat_repo_id: string;
  consistency_mode: string;
  auto_repair: boolean;
  alert_on_inconsistency: boolean;
  created_at: string;
}

export interface CompatExecution {
  execution_id: string;
  job_id: string;
  state: string;
  progress: number;
  files_processed: number;
  bytes_processed: number;
  error_message: string | null;
  started_at: string;
  completed_at: string | null;
}

export interface ImportResult {
  import_id: string;
  status: string;
  resulting_job_id: string | null;
  field_mappings: FieldMapping[];
  unsupported_items: UnsupportedItem[];
  idempotent: boolean;
}

export interface FieldMapping {
  duplicati_field: string;
  hbx_field: string;
  duplicati_value: unknown;
  hbx_value: unknown;
  supported: boolean;
}

export interface UnsupportedItem {
  field: string;
  value: unknown;
  reason: string;
  action: string;
}

export interface CompatMetric {
  metric_id: string;
  name: string;
  value: number;
  labels: Record<string, unknown>;
  recorded_at: string;
}

export const compatRepoApi = {
  list: () => get<{ repositories: CompatRepository[] }>('/compat/repositories'),
  create: (data: unknown) => post<CompatRepository>('/compat/repositories', data),
  update: (id: string, data: unknown) => put(`/compat/repositories/${id}`, data),
  remove: (id: string) => del(`/compat/repositories/${id}`),
  selfCheck: (id: string) => post(`/compat/repositories/${id}/self-check`),
};

export const compatJobApi = {
  list: () => get<{ jobs: CompatJob[] }>('/compat/jobs'),
  create: (data: unknown) => post<CompatJob>('/compat/jobs', data),
  update: (id: string, data: unknown) => put(`/compat/jobs/${id}`, data),
  remove: (id: string) => del(`/compat/jobs/${id}`),
  trigger: (id: string) => post(`/compat/jobs/${id}/trigger`),
  dualCheck: (id: string) => post(`/compat/jobs/${id}/dual-check`),
};

export const dualRepoApi = {
  list: () => get<{ configs: DualRepoConfig[] }>('/compat/dual-repo-configs'),
  create: (data: unknown) => post<DualRepoConfig>('/compat/dual-repo-configs', data),
  remove: (id: string) => del(`/compat/dual-repo-configs/${id}`),
};

export const compatExecApi = {
  list: (jobId?: string) => get<{ executions: CompatExecution[] }>('/compat/executions', { params: { job_id: jobId } }),
  report: (data: unknown) => post('/compat/executions/report', data),
};

export const compatImportApi = {
  import: (data: { format: string; config: string }) => post<ImportResult>('/compat/import', data),
  get: (id: string) => get<ImportResult>(`/compat/import/${id}`),
  list: () => get<{ imports: ImportResult[] }>('/compat/imports'),
};

export const compatMetricApi = {
  list: () => get<{ metrics: CompatMetric[] }>('/compat/metrics'),
};