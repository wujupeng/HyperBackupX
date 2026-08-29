import { get, post, put, del } from './client';
import type {
  LoginRequest, LoginResponse, Device, Policy, Repository,
  BackupJob, BackupVersion, RestoreJob, Alert, AuditLog, AgentLog,
  Role, Organization, DashboardData, User,
  BindPoliciesRequest, JobCreateRequest, JobUpdateRequest,
  RepositoryCreateRequest, RepositoryUpdateRequest, RepositoryVerifyResponse,
  BadouRepository, BadouCreateRepoRequest, BadouVersion, BadouGCReport,
  BadouVerifyResult, BadouNode, BadouClusterHealth,
} from './types';

export const authApi = {
  login: (data: LoginRequest) => post<LoginResponse>('/auth/login', data),
  logout: () => post('/auth/logout'),
};

export const deviceApi = {
  list: () => get<{ devices: Device[] }>('/devices'),
  create: (data: { hostname: string; os_type: string }) => post<Device>('/devices', data),
  remove: (id: string) => del(`/devices/${id}`),
  getPolicies: (id: string) => get<{ policies: Policy[] }>(`/devices/${id}/policies`),
  bindPolicies: (id: string, data: BindPoliciesRequest) => put(`/devices/${id}/policies`, data),
};

export const policyApi = {
  list: () => get<{ policies: Policy[] }>('/policies'),
  create: (data: unknown) => post<Policy>('/policies', data),
  update: (id: string, data: unknown) => put(`/policies/${id}`, data),
  remove: (id: string) => del(`/policies/${id}`),
  versions: (id: string) => get(`/policies/${id}/versions`),
  rollback: (id: string, data: unknown) => post(`/policies/${id}/rollback`, data),
};

export const repositoryApi = {
  list: () => get<{ repositories: Repository[] }>('/repositories'),
  create: (data: RepositoryCreateRequest) => post<Repository>('/repositories', data),
  update: (id: string, data: RepositoryUpdateRequest) => put(`/repositories/${id}`, data),
  remove: (id: string) => del(`/repositories/${id}`),
  verify: (id: string) => post<RepositoryVerifyResponse>(`/repositories/${id}/verify`),
};

export const jobApi = {
  list: () => get<{ jobs: BackupJob[] }>('/jobs'),
  create: (data: JobCreateRequest) => post<BackupJob>('/jobs', data),
  update: (id: string, data: JobUpdateRequest) => put(`/jobs/${id}`, data),
  trigger: (id: string) => post(`/jobs/${id}/trigger`),
};

export const versionApi = {
  list: () => get<{ versions: BackupVersion[] }>('/versions'),
  files: (id: string) => get(`/versions/${id}/files`),
};

export const restoreApi = {
  create: (data: unknown) => post<RestoreJob>('/restores', data),
  list: () => get<{ restores: RestoreJob[] }>('/restores'),
  get: (id: string) => get<RestoreJob>(`/restores/${id}`),
};

export const verifyApi = {
  trigger: (data: unknown) => post('/verify', data),
};

export const monitoringApi = {
  dashboard: () => get<DashboardData>('/monitoring/dashboard'),
  metrics: () => get('/monitoring/metrics'),
};

export const alertApi = {
  list: () => get<{ alerts: Alert[] }>('/alerts'),
  acknowledge: (id: string) => put(`/alerts/${id}`),
};

export const logApi = {
  list: (params?: { device_id?: string; level?: string }) => get<{ logs: AgentLog[] }>('/logs', { params }),
};

export const auditApi = {
  list: () => get<{ audit_logs: AuditLog[] }>('/audit'),
};

export const userApi = {
  list: () => get<{ users: User[] }>('/users'),
  create: (data: unknown) => post<User>('/users', data),
  update: (id: string, data: unknown) => put(`/users/${id}`, data),
  remove: (id: string) => del(`/users/${id}`),
};

export const roleApi = {
  list: () => get<{ roles: Role[] }>('/roles'),
  create: (data: unknown) => post<Role>('/roles', data),
  update: (id: string, data: unknown) => put(`/roles/${id}`, data),
};

export const orgApi = {
  list: () => get<{ organizations: Organization[] }>('/organizations'),
  create: (data: unknown) => post<Organization>('/organizations', data),
  update: (id: string, data: unknown) => put(`/organizations/${id}`, data),
};

export const upgradeApi = {
  agents: (data: unknown) => post('/upgrade/agents', data),
};

export const badouRepoApi = {
  list: () => get<{ repositories: BadouRepository[] }>('/badou/repositories'),
  create: (data: BadouCreateRepoRequest) => post<BadouRepository>('/badou/repositories', data),
  get: (id: string) => get<BadouRepository>(`/badou/repositories/${id}`),
  update: (id: string, data: unknown) => put(`/badou/repositories/${id}`, data),
  remove: (id: string) => del(`/badou/repositories/${id}`),
  setImmutable: (id: string, retentionDays: number) => post(`/badou/repositories/${id}/immutable`, { retention_days: retentionDays }),
  listVersions: (id: string) => get<{ versions: BadouVersion[] }>(`/badou/repositories/${id}/versions`),
  getVersion: (id: string, vid: string) => get<BadouVersion>(`/badou/repositories/${id}/versions/${vid}`),
  deleteVersion: (id: string, vid: string) => del(`/badou/repositories/${id}/versions/${vid}`),
  verify: (id: string, level?: string) => post<BadouVerifyResult>(`/badou/repositories/${id}/verify`, { level: level || 'full' }),
  triggerGC: (id: string) => post<BadouGCReport>(`/badou/repositories/${id}/gc`),
  getGCReport: (id: string) => get<BadouGCReport>(`/badou/repositories/${id}/gc/report`),
};

export const badouClusterApi = {
  listNodes: () => get<{ nodes: BadouNode[] }>('/badou/cluster/nodes'),
  addNode: (data: { node_address: string; node_port?: number; node_role?: string; disk_capacity_bytes?: number }) => post<BadouNode>('/badou/cluster/nodes', data),
  removeNode: (id: string) => del(`/badou/cluster/nodes/${id}`),
  health: () => get<BadouClusterHealth>('/badou/cluster/health'),
  expandCapacity: (data: { node_id: string; additional_bytes: number }) => post('/badou/cluster/capacity', data),
};