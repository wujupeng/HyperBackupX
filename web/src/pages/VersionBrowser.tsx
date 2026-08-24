import { useState, useEffect } from 'react';
import { Card, Table, Input, Space, Tag, Typography, Button, DatePicker } from 'antd';
import { SearchOutlined, EyeOutlined } from '@ant-design/icons';
import { versionApi } from '../api/endpoints';
import type { BackupVersion } from '../api/types';

const { Title } = Typography;
const { RangePicker } = DatePicker;

export default function VersionBrowser() {
  const [versions, setVersions] = useState<BackupVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [dateRange, setDateRange] = useState<[string, string] | null>(null);

  const fetchVersions = async () => {
    setLoading(true);
    try {
      const res = await versionApi.list();
      setVersions(res.versions);
    } catch {
      // API unavailable
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchVersions(); }, []);

  const filtered = versions.filter((v) => {
    if (search && !v.version_id.includes(search) && !v.job_id.includes(search)) return false;
    if (dateRange) {
      const ts = new Date(v.timestamp);
      if (ts < new Date(dateRange[0]) || ts > new Date(dateRange[1])) return false;
    }
    return true;
  });

  return (
    <div>
      <Title level={2}>版本浏览器</Title>
      <Card>
        <Space style={{ marginBottom: 16 }}>
          <Input
            placeholder="搜索版本 ID / 任务 ID"
            prefix={<SearchOutlined />}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            allowClear
          />
          <RangePicker
            onChange={(_, range) => setDateRange(range[0] ? [range[0], range[1]] : null)}
          />
          <Button onClick={fetchVersions} loading={loading}>刷新</Button>
        </Space>

        <Table<BackupVersion>
          rowKey="version_id"
          dataSource={filtered}
          loading={loading}
          pagination={{ pageSize: 20, showSizeChanger: true }}
          columns={[
            { title: '版本号', dataIndex: 'version_number', key: 'version_number', width: 80 },
            {
              title: '时间',
              dataIndex: 'timestamp',
              key: 'timestamp',
              render: (t: string) => new Date(t).toLocaleString(),
              sorter: (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
            },
            {
              title: '类型',
              dataIndex: 'backup_type',
              key: 'backup_type',
              render: (t: string) => (
                <Tag color={t === 'full' ? 'blue' : 'green'}>{t}</Tag>
              ),
            },
            {
              title: '状态',
              dataIndex: 'status',
              key: 'status',
              render: (s: string) => (
                <Tag color={s === 'completed' ? 'success' : s === 'failed' ? 'error' : 'processing'}>{s}</Tag>
              ),
            },
            { title: '文件数', dataIndex: 'file_count', key: 'file_count' },
            { title: '总大小', dataIndex: 'total_size', key: 'total_size', render: (s: number) => formatBytes(s) },
            { title: '存储大小', dataIndex: 'stored_size', key: 'stored_size', render: (s: number) => formatBytes(s) },
            {
              title: '操作',
              key: 'action',
              render: (_, record) => (
                <Button type="link" icon={<EyeOutlined />} href={`/versions/${record.version_id}/files`}>
                  浏览文件
                </Button>
              ),
            },
          ]}
        />
      </Card>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}