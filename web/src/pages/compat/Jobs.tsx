import { useEffect, useState } from 'react';
import { Card, Table, Button, Space, Typography, Tag, Modal, Form, Input, Select, message, Popconfirm } from 'antd';
import { PlusOutlined, ReloadOutlined, DeleteOutlined, PlayCircleOutlined } from '@ant-design/icons';
import { compatJobApi, compatRepoApi, type CompatJob, type CompatRepository } from '../../api/compat';

const { Title } = Typography;

export default function CompatJobsPage() {
  const [jobs, setJobs] = useState<CompatJob[]>([]);
  const [repos, setRepos] = useState<CompatRepository[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [form] = Form.useForm();

  const fetchData = async () => {
    setLoading(true);
    try {
      const [jobsRes, reposRes] = await Promise.all([compatJobApi.list(), compatRepoApi.list()]);
      setJobs(jobsRes.jobs);
      setRepos(reposRes.repositories);
    } catch {
      setJobs([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const handleCreate = async () => {
    const values = await form.validateFields();
    try {
      await compatJobApi.create(values);
      message.success('兼容任务创建成功');
      setCreateOpen(false);
      form.resetFields();
      fetchData();
    } catch {
      message.error('创建失败');
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await compatJobApi.remove(id);
      message.success('已删除');
      fetchData();
    } catch {
      message.error('删除失败');
    }
  };

  const handleTrigger = async (id: string) => {
    try {
      await compatJobApi.trigger(id);
      message.success('任务已触发');
    } catch {
      message.error('触发失败');
    }
  };

  const statusColor: Record<string, string> = { active: 'green', paused: 'orange', disabled: 'red' };

  const columns = [
    { title: '任务名称', dataIndex: 'name', key: 'name' },
    { title: '备份类型', dataIndex: 'backup_type', key: 'backup_type', render: (v: string) => <Tag>{v}</Tag> },
    { title: '双Repo模式', dataIndex: 'dual_repo_mode', key: 'dual_repo_mode', render: (v: string) => <Tag color="blue">{v}</Tag> },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => <Tag color={statusColor[v] || 'default'}>{v}</Tag> },
    { title: '创建时间', dataIndex: 'created_at', key: 'created_at', render: (v: string) => new Date(v).toLocaleString() },
    {
      title: '操作', key: 'action', render: (_: unknown, record: CompatJob) => (
        <Space>
          <Button size="small" icon={<PlayCircleOutlined />} onClick={() => handleTrigger(record.job_id)}>触发</Button>
          <Popconfirm title="确认删除？" onConfirm={() => handleDelete(record.job_id)}>
            <Button size="small" danger icon={<DeleteOutlined />}>删除</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Title level={2}>兼容任务管理</Title>
      <Card>
        <Space style={{ marginBottom: 16 }}>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => setCreateOpen(true)}>新建兼容任务</Button>
          <Button icon={<ReloadOutlined />} onClick={fetchData}>刷新</Button>
        </Space>
        <Table<CompatJob> rowKey="job_id" dataSource={jobs} columns={columns} loading={loading} />
      </Card>

      <Modal title="新建兼容任务" open={createOpen} onOk={handleCreate} onCancel={() => setCreateOpen(false)} width={600}>
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="任务名称" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="repo_id" label="兼容仓库" rules={[{ required: true }]}>
            <Select options={repos.map(r => ({ label: r.name, value: r.repo_id }))} />
          </Form.Item>
          <Form.Item name="backup_type" label="备份类型" initialValue="full">
            <Select options={[{ label: '全量', value: 'full' }, { label: '增量', value: 'incremental' }]} />
          </Form.Item>
          <Form.Item name="dual_repo_mode" label="双Repo模式" initialValue="compatible_only">
            <Select options={[
              { label: '仅兼容', value: 'compatible_only' },
              { label: '仅原生', value: 'native_only' },
              { label: '双Repo一致性', value: 'dual_with_consistency' },
            ]} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}