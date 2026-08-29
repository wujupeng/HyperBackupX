import { useState, useEffect } from 'react';
import { Table, Button, Space, Modal, Form, Input, Select, Popconfirm, message, Typography } from 'antd';
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { repositoryApi } from '../api/endpoints';
import type { Repository, BackendType } from '../api/types';
import { RepoStatusTag, BackendTypeTag } from '../common/StatusTags';
import { formatBytes, formatTimestamp } from '../common/format';
import RepoConnectionForm from './repositories/RepoConnectionForm';

const { Title } = Typography;

const backendOptions: { value: BackendType; label: string }[] = [
  { value: 'local', label: '本地磁盘' },
  { value: 'smb', label: 'SMB/CIFS' },
  { value: 'ftp', label: 'FTP' },
  { value: 'ftps', label: 'FTPS' },
  { value: 'sftp', label: 'SFTP' },
  { value: 'webdav', label: 'WebDAV' },
  { value: 's3', label: 'S3 兼容' },
  { value: 'azure_blob', label: 'Azure Blob' },
  { value: 'gcs', label: 'Google Cloud Storage' },
  { value: 'openstack', label: 'OpenStack Swift' },
];

export default function RepositoriesPage() {
  const [repos, setRepos] = useState<Repository[]>([]);
  const [loading, setLoading] = useState(false);

  const [modalMode, setModalMode] = useState<'create' | 'edit' | null>(null);
  const [editingRepo, setEditingRepo] = useState<Repository | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [form] = Form.useForm();
  const [backendType, setBackendType] = useState<BackendType>('local');

  const [verifyingId, setVerifyingId] = useState<string | null>(null);

  const fetchData = async () => {
    setLoading(true);
    try {
      const res = await repositoryApi.list();
      setRepos(res.repositories);
    } catch {
      setRepos([]);
      message.error('加载仓库列表失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const openCreate = () => {
    setModalMode('create');
    setEditingRepo(null);
    setBackendType('local');
    form.resetFields();
    form.setFieldsValue({ backend_type: 'local' });
  };

  const openEdit = (record: Repository) => {
    setModalMode('edit');
    setEditingRepo(record);
    const bt = record.backend_type as BackendType;
    setBackendType(bt);
    form.setFieldsValue({
      name: record.name,
      backend_type: bt,
    });
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      const connectionConfig = values.connection_config || {};
      const cleanedConfig: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(connectionConfig)) {
        if (v !== undefined && v !== '' && v !== null) {
          cleanedConfig[k] = v;
        }
      }

      if (modalMode === 'create') {
        await repositoryApi.create({
          name: values.name,
          backend_type: values.backend_type,
          connection_config: cleanedConfig,
        });
        message.success('创建成功');
      } else if (modalMode === 'edit' && editingRepo) {
        await repositoryApi.update(editingRepo.repository_id, {
          name: values.name,
          connection_config: cleanedConfig,
        });
        message.success('编辑成功');
      }
      setModalMode(null);
      form.resetFields();
      fetchData();
    } catch {
      if (modalMode === 'create') {
        message.error('创建失败');
      } else {
        message.error('编辑失败');
      }
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await repositoryApi.remove(id);
      message.success('删除成功');
      fetchData();
    } catch {
      message.error('删除失败');
    }
  };

  const handleVerify = async (id: string) => {
    setVerifyingId(id);
    try {
      const result = await repositoryApi.verify(id);
      if (result.reachable) {
        message.success('验证通过');
      } else {
        message.error('验证失败: ' + (result.message || '不可达'));
      }
    } catch {
      message.error('验证失败');
    } finally {
      setVerifyingId(null);
    }
  };

  const columns = [
    { title: '仓库ID', dataIndex: 'repository_id', key: 'repository_id', render: (v: string) => v.slice(0, 8) },
    { title: '名称', dataIndex: 'name', key: 'name' },
    { title: '后端类型', dataIndex: 'backend_type', key: 'backend_type', render: (v: string) => <BackendTypeTag type={v} /> },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => <RepoStatusTag status={v} /> },
    { title: '已用容量', dataIndex: 'used_capacity', key: 'used_capacity', render: (v: number | null) => formatBytes(v) },
    { title: '总容量', dataIndex: 'total_capacity', key: 'total_capacity', render: (v: number | null) => formatBytes(v) },
    { title: '创建时间', dataIndex: 'created_at', key: 'created_at', render: (v?: string) => formatTimestamp(v) },
    {
      title: '操作', key: 'action', render: (_: unknown, record: Repository) => (
        <Space>
          <Button size="small" onClick={() => openEdit(record)}>编辑</Button>
          <Button size="small" loading={verifyingId === record.repository_id} onClick={() => handleVerify(record.repository_id)}>验证</Button>
          <Popconfirm title="确定删除此仓库？" onConfirm={() => handleDelete(record.repository_id)}>
            <Button size="small" danger>删除</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Title level={2}>仓库管理</Title>
      <Space style={{ marginBottom: 16 }}>
        <Button icon={<ReloadOutlined />} onClick={fetchData}>刷新</Button>
        <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>新建仓库</Button>
      </Space>
      <Table
        columns={columns}
        dataSource={repos}
        rowKey="repository_id"
        loading={loading}
        pagination={{ pageSize: 10, showSizeChanger: true, pageSizeOptions: ['10', '20', '50'] }}
        locale={{ emptyText: '暂无仓库，请创建新仓库' }}
      />
      <Modal
        title={modalMode === 'create' ? '新建仓库' : '编辑仓库'}
        open={modalMode !== null}
        onOk={handleSubmit}
        onCancel={() => { setModalMode(null); form.resetFields(); }}
        confirmLoading={submitting}
        width={720}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="仓库名称" rules={[{ required: true, message: '请输入仓库名称' }, { max: 100, message: '最多 100 个字符' }]}>
            <Input />
          </Form.Item>
          <Form.Item name="backend_type" label="后端类型" rules={[{ required: true, message: '请选择后端类型' }]}>
            <Select
              options={backendOptions}
              disabled={modalMode === 'edit'}
              onChange={(v: BackendType) => setBackendType(v)}
            />
          </Form.Item>
          <RepoConnectionForm
            backendType={backendType}
            form={form}
            initialConfig={editingRepo?.connection_config}
            isEdit={modalMode === 'edit'}
          />
        </Form>
      </Modal>
    </div>
  );
}