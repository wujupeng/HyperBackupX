import { useState, useEffect } from 'react';
import {
  Card, Table, Input, Space, Button, Typography, Tag, DatePicker, Alert,
} from 'antd';
import { SearchOutlined, ReloadOutlined } from '@ant-design/icons';
import { auditApi } from '../api/endpoints';
import type { AuditLog } from '../api/types';

const { Title } = Typography;
const { RangePicker } = DatePicker;

const resultColors: Record<string, string> = {
  success: 'success',
  failed: 'error',
};

export default function AuditPage() {
  const [logs, setLogs] = useState<AuditLog[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [actionFilter, setActionFilter] = useState('');

  const fetchLogs = async () => {
    setLoading(true);
    try {
      const res = await auditApi.list();
      setLogs(res.audit_logs);
    } catch {
      setLogs([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchLogs(); }, []);

  const filtered = logs.filter((log) => {
    if (search && !log.actor_id.includes(search) && !log.target_id.includes(search)) return false;
    if (actionFilter && !log.action.includes(actionFilter)) return false;
    return true;
  });

  return (
    <div>
      <Title level={2}>审计日志</Title>
      <Alert
        message="审计日志为只读"
        description="审计日志记录所有关键操作，仅支持查询，不可修改或删除。"
        type="info"
        style={{ marginBottom: 16 }}
      />
      <Card>
        <Space wrap style={{ marginBottom: 16 }}>
          <Input
            placeholder="搜索操作者/目标 ID"
            prefix={<SearchOutlined />}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{ width: 250 }}
            allowClear
          />
          <Input
            placeholder="操作类型"
            value={actionFilter}
            onChange={(e) => setActionFilter(e.target.value)}
            style={{ width: 150 }}
            allowClear
          />
          <RangePicker />
          <Button icon={<ReloadOutlined />} onClick={fetchLogs} loading={loading}>刷新</Button>
        </Space>

        <Table<AuditLog>
          rowKey="log_id"
          dataSource={filtered}
          loading={loading}
          pagination={{ pageSize: 50, showSizeChanger: true }}
          scroll={{ x: 900 }}
          columns={[
            { title: '时间', dataIndex: 'timestamp', key: 'timestamp', width: 180, render: (t: string) => new Date(t).toLocaleString(), sorter: (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime() },
            { title: '操作者', dataIndex: 'actor_id', key: 'actor_id', width: 120, render: (id: string) => id.slice(0, 8) },
            { title: '操作', dataIndex: 'action', key: 'action', width: 100 },
            { title: '目标类型', dataIndex: 'target_type', key: 'target_type', width: 100 },
            { title: '目标 ID', dataIndex: 'target_id', key: 'target_id', width: 120, render: (id: string) => id.slice(0, 8) },
            { title: '结果', dataIndex: 'result', key: 'result', width: 80, render: (r: string) => <Tag color={resultColors[r] || 'default'}>{r}</Tag> },
            { title: 'Trace ID', dataIndex: 'trace_id', key: 'trace_id', width: 120, render: (t: string | null) => t ? t.slice(0, 8) : '-' },
          ]}
        />
      </Card>
    </div>
  );
}