import { useState, useEffect } from 'react';
import { Table, Button, Space, Modal, Form, Input, InputNumber, Popconfirm, message, Typography, Tag } from 'antd';
import { PlusOutlined, ReloadOutlined, DeleteOutlined } from '@ant-design/icons';
import { badouRepoApi } from '../../api/endpoints';
import type { BadouRepository, BadouVersion } from '../../api/types';
import { formatBytes, formatTimestamp } from '../../common/format';

const { Title } = Typography;

function statusTag(status: string) {
  const colors: Record<string, string> = { active: 'green', disabled: 'default', error: 'red', maintenance: 'orange' };
  return <Tag color={colors[status] || 'default'}>{status}</Tag>;
}

export default function BadouRepositoriesPage() {
  const [repos, setRepos] = useState<BadouRepository[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [form] = Form.useForm();
  const [immutableModalOpen, setImmutableModalOpen] = useState(false);
  const [immutableRepo, setImmutableRepo] = useState<BadouRepository | null>(null);
  const [immutableDays, setImmutableDays] = useState(0);
  const [versionsModalOpen, setVersionsModalOpen] = useState(false);
  const [versions, setVersions] = useState<BadouVersion[]>([]);
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [verifyingId, setVerifyingId] = useState<string | null>(null);
  const [gcId, setGcId] = useState<string | null>(null);

  const fetchData = async () => {
    setLoading(true);
    try {
      const res = await badouRepoApi.list();
      setRepos(res.repositories || []);
    } catch {
      setRepos([]);
      message.error('加载八斗仓库列表失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const handleCreate = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      await badouRepoApi.create(values);
      message.success('创建成功');
      setModalOpen(false);
      form.resetFields();
      fetchData();
    } catch {
      message.error('创建失败');
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await badouRepoApi.remove(id);
      message.success('删除成功');
      fetchData();
    } catch {
      message.error('删除失败');
    }
  };

  const handleSetImmutable = async () => {
    if (!immutableRepo) return;
    try {
      setSubmitting(true);
      await badouRepoApi.setImmutable(immutableRepo.repo_id, immutableDays);
      message.success('不可变保留设置成功');
      setImmutableModalOpen(false);
      fetchData();
    } catch {
      message.error('设置失败');
    } finally {
      setSubmitting(false);
    }
  };

  const handleListVersions = async (repo: BadouRepository) => {
    setVersionsModalOpen(true);
    setVersionsLoading(true);
    try {
      const res = await badouRepoApi.listVersions(repo.repo_id);
      setVersions(res.versions || []);
    } catch {
      setVersions([]);
      message.error('加载版本列表失败');
    } finally {
      setVersionsLoading(false);
    }
  };

  const handleVerify = async (id: string) => {
    setVerifyingId(id);
    try {
      const result = await badouRepoApi.verify(id);
      if (result.passed) {
        message.success(`校验通过 (错误: ${result.errors}, 警告: ${result.warnings})`);
      } else {
        message.error(`校验失败 (错误: ${result.errors}, 警告: ${result.warnings})`);
      }
    } catch {
      message.error('校验失败');
    } finally {
      setVerifyingId(null);
    }
  };

  const handleGC = async (id: string) => {
    setGcId(id);
    try {
      const report = await badouRepoApi.triggerGC(id);
      message.success(`GC 完成: 扫描 ${report.chunks_scanned}, 删除 ${report.chunks_deleted}, 释放 ${formatBytes(report.bytes_freed)}`);
      fetchData();
    } catch {
      message.error('GC 失败');
    } finally {
      setGcId(null);
    }
  };

  const columns = [
    { title: 'ID', dataIndex: 'repo_id', key: 'repo_id', render: (v: string) => v.slice(0, 8) },
    { title: '名称', dataIndex: 'name', key: 'name' },
    { title: '节点地址', key: 'addr', render: (_: unknown, r: BadouRepository) => `${r.node_address}:${r.node_port}` },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => statusTag(v) },
    { title: '不可变保留(天)', dataIndex: 'immutable_retention_days', key: 'immutable' },
    { title: '创建时间', dataIndex: 'created_at', key: 'created_at', render: (v: string) => formatTimestamp(v) },
    {
      title: '操作', key: 'action', render: (_: unknown, r: BadouRepository) => (
        <Space>
          <Button size="small" onClick={() => handleListVersions(r)}>版本</Button>
          <Button size="small" onClick={() => { setImmutableRepo(r); setImmutableDays(r.immutable_retention_days); setImmutableModalOpen(true); }}>不可变</Button>
          <Button size="small" loading={verifyingId === r.repo_id} onClick={() => handleVerify(r.repo_id)}>校验</Button>
          <Button size="small" loading={gcId === r.repo_id} onClick={() => handleGC(r.repo_id)}>GC</Button>
          <Popconfirm title="确定删除？" onConfirm={() => handleDelete(r.repo_id)}>
            <Button size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const versionColumns = [
    { title: 'Version ID', dataIndex: 'version_id', key: 'version_id', render: (v: string) => v.slice(0, 12) },
    { title: '大小', dataIndex: 'size', key: 'size', render: (v: number) => formatBytes(v) },
    { title: 'Chunk 数', dataIndex: 'chunk_count', key: 'chunk_count' },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => statusTag(v) },
    { title: '创建时间', dataIndex: 'created_at', key: 'created_at', render: (v: string) => formatTimestamp(v) },
  ];

  return (
    <div>
      <Title level={2}>八斗 Repository 管理</Title>
      <Space style={{ marginBottom: 16 }}>
        <Button icon={<ReloadOutlined />} onClick={fetchData}>刷新</Button>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => { form.resetFields(); setModalOpen(true); }}>注册仓库</Button>
      </Space>
      <Table
        columns={columns}
        dataSource={repos}
        rowKey="repo_id"
        loading={loading}
        pagination={{ pageSize: 10 }}
        locale={{ emptyText: '暂无八斗仓库' }}
      />

      <Modal title="注册八斗仓库" open={modalOpen} onOk={handleCreate} onCancel={() => setModalOpen(false)} confirmLoading={submitting} width={640}>
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="仓库名称" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="description" label="描述">
            <Input />
          </Form.Item>
          <Form.Item name="node_address" label="节点地址" rules={[{ required: true }]}>
            <Input placeholder="192.168.1.60" />
          </Form.Item>
          <Form.Item name="node_port" label="节点端口" initialValue={50051}>
            <InputNumber min={1} max={65535} />
          </Form.Item>
          <Form.Item name="tls_cert_path" label="TLS 证书路径">
            <Input placeholder="/etc/badou/tls/client.crt" />
          </Form.Item>
          <Form.Item name="tls_key_path" label="TLS 私钥路径">
            <Input placeholder="/etc/badou/tls/client.key" />
          </Form.Item>
          <Form.Item name="tls_ca_path" label="TLS CA 路径">
            <Input placeholder="/etc/badou/tls/ca.crt" />
          </Form.Item>
          <Form.Item name="jwt_subject" label="JWT Subject">
            <Input />
          </Form.Item>
          <Form.Item name="jwt_secret_ref" label="JWT Secret 引用">
            <Input />
          </Form.Item>
          <Form.Item name="immutable_retention_days" label="不可变保留天数" initialValue={0}>
            <InputNumber min={0} />
          </Form.Item>
        </Form>
      </Modal>

      <Modal title="设置不可变保留" open={immutableModalOpen} onOk={handleSetImmutable} onCancel={() => setImmutableModalOpen(false)} confirmLoading={submitting}>
        <Space direction="vertical" style={{ width: '100%' }}>
          <Typography.Text>仓库: {immutableRepo?.name}</Typography.Text>
          <InputNumber value={immutableDays} onChange={(v) => setImmutableDays(v || 0)} min={0} max={36500} style={{ width: '100%' }} addonAfter="天" />
          <Typography.Text type="secondary">设置后，保留期内的 Version 不可删除</Typography.Text>
        </Space>
      </Modal>

      <Modal title="版本列表" open={versionsModalOpen} onCancel={() => setVersionsModalOpen(false)} footer={null} width={800}>
        <Table columns={versionColumns} dataSource={versions} rowKey="version_id" loading={versionsLoading} pagination={{ pageSize: 10 }} size="small" />
      </Modal>
    </div>
  );
}