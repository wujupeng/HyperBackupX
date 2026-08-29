import { useState, useEffect, useMemo } from 'react';
import { Table, Button, Space, Modal, Form, Input, Select, Popconfirm, Drawer, message, Typography } from 'antd';
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { jobApi, deviceApi, versionApi } from '../api/endpoints';
import type { BackupJob, BackupVersion, Device } from '../api/types';
import { JobStatusTag } from '../common/StatusTags';
import { formatTimestamp, formatBytes } from '../common/format';

const { Title } = Typography;

export default function JobsPage() {
  const [jobs, setJobs] = useState<BackupJob[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);

  const [modalMode, setModalMode] = useState<'create' | 'edit' | null>(null);
  const [editingJob, setEditingJob] = useState<BackupJob | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [form] = Form.useForm();

  const [triggeringId, setTriggeringId] = useState<string | null>(null);

  const [versionDrawerJobId, setVersionDrawerJobId] = useState<string | null>(null);
  const [versions, setVersions] = useState<BackupVersion[]>([]);
  const [versionsLoading, setVersionsLoading] = useState(false);

  const fetchData = async () => {
    setLoading(true);
    try {
      const [jobsRes, devicesRes] = await Promise.all([
        jobApi.list(),
        deviceApi.list(),
      ]);
      setJobs(jobsRes.jobs);
      setDevices(devicesRes.devices);
    } catch {
      setJobs([]);
      setDevices([]);
      message.error('加载任务列表失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const deviceMap = useMemo(() => {
    const m = new Map<string, Device>();
    devices.forEach(d => m.set(d.device_id, d));
    return m;
  }, [devices]);

  const openCreate = () => {
    setModalMode('create');
    setEditingJob(null);
    form.resetFields();
  };

  const openEdit = (record: BackupJob) => {
    setModalMode('edit');
    setEditingJob(record);
    form.setFieldsValue({
      name: record.name,
      device_id: record.device_id,
    });
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      const payload = {
        name: values.name,
        device_id: values.device_id,
        backup_config: values.backup_config ? JSON.parse(values.backup_config) : undefined,
      };
      if (modalMode === 'create') {
        await jobApi.create(payload);
        message.success('创建成功');
      } else if (modalMode === 'edit' && editingJob) {
        await jobApi.update(editingJob.job_id, payload);
        message.success('编辑成功');
      }
      setModalMode(null);
      form.resetFields();
      fetchData();
    } catch (err) {
      if (err instanceof SyntaxError) {
        message.error('备份配置不是有效的 JSON');
      } else if (modalMode === 'create') {
        message.error('创建失败');
      } else {
        message.error('编辑失败');
      }
    } finally {
      setSubmitting(false);
    }
  };

  const handleTrigger = async (id: string) => {
    setTriggeringId(id);
    try {
      await jobApi.trigger(id);
      message.success('任务已触发');
    } catch {
      message.error('触发失败');
    } finally {
      setTriggeringId(null);
    }
  };

  const openVersionDrawer = async (jobId: string) => {
    setVersionDrawerJobId(jobId);
    setVersionsLoading(true);
    try {
      const res = await versionApi.list();
      setVersions(res.versions);
    } catch {
      setVersions([]);
      message.error('加载版本失败');
    } finally {
      setVersionsLoading(false);
    }
  };

  const columns = [
    { title: '任务ID', dataIndex: 'job_id', key: 'job_id', render: (v: string) => v.slice(0, 8) },
    { title: '关联设备', dataIndex: 'device_id', key: 'device_id', render: (v: string) => deviceMap.get(v)?.hostname || v },
    { title: '任务名称', dataIndex: 'name', key: 'name' },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => <JobStatusTag status={v} /> },
    { title: '创建时间', dataIndex: 'created_at', key: 'created_at', render: (v: string) => formatTimestamp(v) },
    {
      title: '操作', key: 'action', render: (_: unknown, record: BackupJob) => (
        <Space>
          <Button size="small" onClick={() => openEdit(record)}>编辑</Button>
          <Popconfirm title="确定触发此任务？" onConfirm={() => handleTrigger(record.job_id)}>
            <Button size="small" type="primary" loading={triggeringId === record.job_id}>触发</Button>
          </Popconfirm>
          <Button size="small" onClick={() => openVersionDrawer(record.job_id)}>查看版本</Button>
        </Space>
      ),
    },
  ];

  const versionColumns = [
    { title: '版本号', dataIndex: 'version_number', key: 'version_number' },
    { title: '时间', dataIndex: 'timestamp', key: 'timestamp', render: (v: string) => formatTimestamp(v) },
    { title: '备份类型', dataIndex: 'backup_type', key: 'backup_type' },
    { title: '文件数', dataIndex: 'file_count', key: 'file_count' },
    { title: '原始大小', dataIndex: 'total_size', key: 'total_size', render: (v: number) => formatBytes(v) },
    { title: '存储大小', dataIndex: 'stored_size', key: 'stored_size', render: (v: number) => formatBytes(v) },
    { title: '状态', dataIndex: 'status', key: 'status' },
  ];

  const filteredVersions = versionDrawerJobId
    ? versions.filter(v => v.job_id === versionDrawerJobId)
    : [];

  return (
    <div>
      <Title level={2}>备份任务</Title>
      <Space style={{ marginBottom: 16 }}>
        <Button icon={<ReloadOutlined />} onClick={fetchData}>刷新</Button>
        <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>新建任务</Button>
      </Space>
      <Table
        columns={columns}
        dataSource={jobs}
        rowKey="job_id"
        loading={loading}
        pagination={{ pageSize: 10, showSizeChanger: true, pageSizeOptions: ['10', '20', '50'] }}
        locale={{ emptyText: '暂无备份任务，请创建新任务' }}
      />
      <Modal
        title={modalMode === 'create' ? '新建任务' : '编辑任务'}
        open={modalMode !== null}
        onOk={handleSubmit}
        onCancel={() => { setModalMode(null); form.resetFields(); }}
        confirmLoading={submitting}
        width={520}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="任务名称" rules={[{ required: true, message: '请输入任务名称' }, { max: 100, message: '最多 100 个字符' }]}>
            <Input />
          </Form.Item>
          <Form.Item name="device_id" label="关联设备" rules={[{ required: true, message: '请选择关联设备' }]}>
            <Select
              options={devices.map(d => ({ value: d.device_id, label: d.hostname }))}
              placeholder="请选择设备"
            />
          </Form.Item>
          <Form.Item name="backup_config" label="备份配置（可选 JSON）">
            <Input.TextArea rows={4} placeholder='{"paths": ["/home/user/documents"], "excludes": []}' />
          </Form.Item>
        </Form>
      </Modal>
      <Drawer
        title="版本查看"
        open={versionDrawerJobId !== null}
        onClose={() => setVersionDrawerJobId(null)}
        width={720}
      >
        <Table
          columns={versionColumns}
          dataSource={filteredVersions}
          rowKey="version_id"
          loading={versionsLoading}
          pagination={{ pageSize: 10, showSizeChanger: true, pageSizeOptions: ['10', '20', '50'] }}
          locale={{ emptyText: '暂无版本' }}
        />
      </Drawer>
    </div>
  );
}