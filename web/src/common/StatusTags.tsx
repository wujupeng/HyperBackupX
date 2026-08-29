import { Tag } from 'antd';

export function DeviceStatusTag({ status }: { status: string }) {
  const map: Record<string, string> = { online: 'green', offline: 'default', disabled: 'red' };
  return <Tag color={map[status] || 'default'}>{status}</Tag>;
}

export function JobStatusTag({ status }: { status: string }) {
  const map: Record<string, string> = { active: 'green', paused: 'orange', disabled: 'default', failed: 'red' };
  return <Tag color={map[status] || 'default'}>{status}</Tag>;
}

export function RepoStatusTag({ status }: { status: string }) {
  const map: Record<string, string> = { active: 'green', verified: 'green', error: 'red', pending: 'orange' };
  return <Tag color={map[status] || 'default'}>{status}</Tag>;
}

export function BackendTypeTag({ type }: { type: string }) {
  return <Tag color="blue">{type}</Tag>;
}