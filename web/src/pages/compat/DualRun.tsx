import { useEffect, useState } from 'react';
import { Card, Table, Button, Space, Typography, Tag, Descriptions, Collapse } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';
import { compatExecApi, type CompatExecution } from '../../api/compat';

const { Title } = Typography;

interface DualRunResult {
  version_id: string;
  native_result: string;
  compat_result: string;
  sha256_match: boolean;
  file_count: number;
  total_size: number;
  deviations: string[];
}

const mockDualRuns: DualRunResult[] = [
  { version_id: 'v-001', native_result: 'success', compat_result: 'success', sha256_match: true, file_count: 120, total_size: 5242880, deviations: [] },
  { version_id: 'v-002', native_result: 'success', compat_result: 'success', sha256_match: true, file_count: 98, total_size: 4194304, deviations: [] },
  { version_id: 'v-003', native_result: 'success', compat_result: 'success', sha256_match: false, file_count: 105, total_size: 6291456, deviations: ['metadata timestamp differs'] },
];

export default function DualRunPage() {
  const [executions, setExecutions] = useState<CompatExecution[]>([]);
  const [loading, setLoading] = useState(false);
  const [dualRuns] = useState<DualRunResult[]>(mockDualRuns);

  const fetchData = async () => {
    setLoading(true);
    try {
      const res = await compatExecApi.list();
      setExecutions(res.executions);
    } catch {
      setExecutions([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const columns = [
    { title: '版本', dataIndex: 'version_id', key: 'version_id' },
    { title: '原生结果', dataIndex: 'native_result', key: 'native_result', render: (v: string) => <Tag color={v === 'success' ? 'green' : 'red'}>{v}</Tag> },
    { title: '兼容结果', dataIndex: 'compat_result', key: 'compat_result', render: (v: string) => <Tag color={v === 'success' ? 'green' : 'red'}>{v}</Tag> },
    { title: 'SHA-256', dataIndex: 'sha256_match', key: 'sha256_match', render: (v: boolean) => <Tag color={v ? 'green' : 'red'}>{v ? '一致' : '不一致'}</Tag> },
    { title: '文件数', dataIndex: 'file_count', key: 'file_count' },
    { title: '总大小', dataIndex: 'total_size', key: 'total_size', render: (v: number) => `${(v / 1024 / 1024).toFixed(2)} MB` },
    { title: '偏离项', dataIndex: 'deviations', key: 'deviations', render: (v: string[]) => v.length === 0 ? <Tag color="green">无</Tag> : <Tag color="orange">{v.length} 项</Tag> },
  ];

  return (
    <div>
      <Title level={2}>双跑报告</Title>

      <Card title="双跑结果列表" style={{ marginBottom: 16 }}>
        <Space style={{ marginBottom: 16 }}>
          <Button icon={<ReloadOutlined />} onClick={fetchData} loading={loading}>刷新</Button>
        </Space>
        <Table<DualRunResult> rowKey="version_id" dataSource={dualRuns} columns={columns} />
      </Card>

      <Card title="偏离详情">
        <Collapse items={dualRuns.filter(r => r.deviations.length > 0).map(r => ({
          key: r.version_id,
          label: `版本 ${r.version_id}`,
          children: (
            <Descriptions column={1}>
              <Descriptions.Item label="偏离项">{r.deviations.join(', ')}</Descriptions.Item>
              <Descriptions.Item label="SHA-256 一致">{r.sha256_match ? '是' : '否'}</Descriptions.Item>
              <Descriptions.Item label="文件数">{r.file_count}</Descriptions.Item>
            </Descriptions>
          ),
        }))} />
        {dualRuns.every(r => r.deviations.length === 0) && <Tag color="green">所有版本无偏离</Tag>}
      </Card>

      <Card title="兼容执行记录" style={{ marginTop: 16 }}>
        <Table<CompatExecution> rowKey="execution_id" dataSource={executions} loading={loading} columns={[
          { title: '执行ID', dataIndex: 'execution_id', key: 'execution_id', render: (v: string) => v.substring(0, 8) + '...' },
          { title: '状态', dataIndex: 'state', key: 'state', render: (v: string) => <Tag color={v === 'success' ? 'green' : v === 'failed' ? 'red' : 'blue'}>{v}</Tag> },
          { title: '进度', dataIndex: 'progress', key: 'progress', render: (v: number) => `${(v * 100).toFixed(1)}%` },
          { title: '开始时间', dataIndex: 'started_at', key: 'started_at', render: (v: string) => new Date(v).toLocaleString() },
        ]} />
      </Card>
    </div>
  );
}