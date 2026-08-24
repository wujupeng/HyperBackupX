import { useState, useEffect } from 'react';
import {
  Card, Table, Input, Select, Space, Button, Typography, Tag, DatePicker,
} from 'antd';
import { SearchOutlined, ReloadOutlined } from '@ant-design/icons';
import { logApi } from '../api/endpoints';
import type { AgentLog } from '../api/types';

const { Title } = Typography;
const { RangePicker } = DatePicker;

const levelColors: Record<string, string> = {
  error: 'error',
  warn: 'warning',
  info: 'processing',
  debug: 'default',
  trace: 'default',
};

export default function LogsPage() {
  const [logs, setLogs] = useState<AgentLog[]>([]);
  const [loading, setLoading] = useState(false);
  const [deviceId, setDeviceId] = useState<string>('');
  const [level, setLevel] = useState<string>('');
  const [traceId, setTraceId] = useState<string>('');
  const [search, setSearch] = useState<string>('');

  const fetchLogs = async () => {
    setLoading(true);
    try {
      const params: { device_id?: string; level?: string } = {};
      if (deviceId) params.device_id = deviceId;
      if (level) params.level = level;
      const res = await logApi.list(params);
      setLogs(res.logs);
    } catch {
      setLogs([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchLogs(); }, []);

  const filtered = logs.filter((log) => {
    if (traceId && !log.trace_id?.includes(traceId)) return false;
    if (search && !log.message.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  return (
    <div>
      <Title level={2}>日志检索</Title>
      <Card>
        <Space wrap style={{ marginBottom: 16 }}>
          <Input
            placeholder="设备 ID"
            value={deviceId}
            onChange={(e) => setDeviceId(e.target.value)}
            style={{ width: 200 }}
            allowClear
          />
          <Select
            placeholder="日志级别"
            value={level || undefined}
            onChange={(v) => setLevel(v || '')}
            style={{ width: 120 }}
            allowClear
            options={[
              { label: 'ERROR', value: 'error' },
              { label: 'WARN', value: 'warn' },
              { label: 'INFO', value: 'info' },
              { label: 'DEBUG', value: 'debug' },
              { label: 'TRACE', value: 'trace' },
            ]}
          />
          <Input
            placeholder="Trace ID"
            value={traceId}
            onChange={(e) => setTraceId(e.target.value)}
            style={{ width: 200 }}
            allowClear
          />
          <Input
            placeholder="搜索消息"
            prefix={<SearchOutlined />}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{ width: 250 }}
            allowClear
          />
          <RangePicker />
          <Button type="primary" icon={<SearchOutlined />} onClick={fetchLogs} loading={loading}>
            查询
          </Button>
          <Button icon={<ReloadOutlined />} onClick={fetchLogs}>刷新</Button>
        </Space>

        <Table<AgentLog>
          rowKey="log_id"
          dataSource={filtered}
          loading={loading}
          pagination={{ pageSize: 50, showSizeChanger: true }}
          scroll={{ x: 800 }}
          columns={[
            { title: '时间', dataIndex: 'timestamp', key: 'timestamp', width: 180, render: (t: string) => new Date(t).toLocaleString(), sorter: (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime() },
            { title: '级别', dataIndex: 'level', key: 'level', width: 80, render: (l: string) => <Tag color={levelColors[l] || 'default'}>{l.toUpperCase()}</Tag> },
            { title: '设备', dataIndex: 'device_id', key: 'device_id', width: 120, render: (id: string) => id.slice(0, 8) },
            { title: '组件', dataIndex: 'component', key: 'component', width: 120 },
            { title: 'Trace ID', dataIndex: 'trace_id', key: 'trace_id', width: 120, render: (t: string | null) => t ? t.slice(0, 8) : '-' },
            { title: '消息', dataIndex: 'message', key: 'message', ellipsis: true },
          ]}
        />
      </Card>
    </div>
  );
}