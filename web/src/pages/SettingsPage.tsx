import { useState, useEffect, useCallback } from 'react';
import {
  Card, Table, Button, Typography, Space, Modal, Form, Input, Select,
  Tag, Popconfirm, message, Tabs,
} from 'antd';
import { PlusOutlined, EditOutlined, DeleteOutlined, ReloadOutlined } from '@ant-design/icons';
import {
  repositoryApi, policyApi, userApi, roleApi, orgApi,
} from '../api/endpoints';
import type { Repository, Policy, User, Role, Organization } from '../api/types';

const { Title } = Typography;

export default function SettingsPage() {
  return (
    <div>
      <Title level={2}>系统设置</Title>
      <Card>
        <Tabs
          items={[
            { key: 'repos', label: '仓库管理', children: <RepoTab /> },
            { key: 'policies', label: '策略管理', children: <PolicyTab /> },
            { key: 'users', label: '用户管理', children: <UserTab /> },
            { key: 'roles', label: '角色管理', children: <RoleTab /> },
            { key: 'orgs', label: '组织管理', children: <OrgTab /> },
          ]}
        />
      </Card>
    </div>
  );
}

function RepoTab() {
  const [repos, setRepos] = useState<Repository[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [form] = Form.useForm();

  const fetch = useCallback(async () => {
    setLoading(true);
    try { setRepos((await repositoryApi.list()).repositories); } catch { setRepos([]); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);

  const onCreate = async () => {
    const values = await form.validateFields();
    await repositoryApi.create(values);
    message.success('仓库创建成功');
    setModalOpen(false);
    form.resetFields();
    fetch();
  };

  return (
    <>
      <Space style={{ marginBottom: 16 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => setModalOpen(true)}>添加仓库</Button>
        <Button icon={<ReloadOutlined />} onClick={fetch}>刷新</Button>
      </Space>
      <Table<Repository>
        rowKey="repository_id"
        dataSource={repos}
        loading={loading}
        columns={[
          { title: '名称', dataIndex: 'name', key: 'name' },
          { title: '类型', dataIndex: 'backend_type', key: 'backend_type' },
          { title: '状态', dataIndex: 'status', key: 'status', render: (s: string) => <Tag color={s === 'active' ? 'success' : 'default'}>{s}</Tag> },
          { title: '已用', dataIndex: 'used_capacity', key: 'used', render: (v: number | null) => v ? formatBytes(v) : '-' },
          { title: '总量', dataIndex: 'total_capacity', key: 'total', render: (v: number | null) => v ? formatBytes(v) : '-' },
          {
            title: '操作', key: 'action', render: (_, r) => (
              <Popconfirm title="确认删除？" onConfirm={async () => { await repositoryApi.remove(r.repository_id); message.success('已删除'); fetch(); }}>
                <Button danger size="small" icon={<DeleteOutlined />}>删除</Button>
              </Popconfirm>
            ),
          },
        ]}
      />
      <Modal title="添加仓库" open={modalOpen} onOk={onCreate} onCancel={() => setModalOpen(false)}>
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="名称" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="backend_type" label="类型" rules={[{ required: true }]}>
            <Select options={[{ label: '本地', value: 'local' }, { label: 'S3', value: 's3' }, { label: 'WebDAV', value: 'webdav' }, { label: 'SFTP', value: 'sftp' }, { label: 'FTP', value: 'ftp' }, { label: 'SMB', value: 'smb' }]} />
          </Form.Item>
          <Form.Item name="connection_config" label="连接配置 (JSON)"><Input.TextArea rows={4} placeholder='{"host":"...","port":...}' /></Form.Item>
        </Form>
      </Modal>
    </>
  );
}

function PolicyTab() {
  const [policies, setPolicies] = useState<Policy[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [form] = Form.useForm();

  const fetch = useCallback(async () => {
    setLoading(true);
    try { setPolicies((await policyApi.list()).policies); } catch { setPolicies([]); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);

  const onCreate = async () => {
    const values = await form.validateFields();
    await policyApi.create(values);
    message.success('策略创建成功');
    setModalOpen(false);
    form.resetFields();
    fetch();
  };

  return (
    <>
      <Space style={{ marginBottom: 16 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => setModalOpen(true)}>添加策略</Button>
        <Button icon={<ReloadOutlined />} onClick={fetch}>刷新</Button>
      </Space>
      <Table<Policy>
        rowKey="policy_id"
        dataSource={policies}
        loading={loading}
        columns={[
          { title: '名称', dataIndex: 'name', key: 'name' },
          { title: '版本', dataIndex: 'version', key: 'version' },
          { title: '范围', dataIndex: 'scope_type', key: 'scope_type' },
          { title: '状态', dataIndex: 'status', key: 'status', render: (s: string) => <Tag color={s === 'active' ? 'success' : 'default'}>{s}</Tag> },
          {
            title: '操作', key: 'action', render: (_, r) => (
              <Space>
                <Button size="small" icon={<EditOutlined />} onClick={() => message.info('编辑功能待实现')} />
                <Popconfirm title="确认删除？" onConfirm={async () => { await policyApi.remove(r.policy_id); message.success('已删除'); fetch(); }}>
                  <Button danger size="small" icon={<DeleteOutlined />} />
                </Popconfirm>
              </Space>
            ),
          },
        ]}
      />
      <Modal title="添加策略" open={modalOpen} onOk={onCreate} onCancel={() => setModalOpen(false)}>
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="名称" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="scope_type" label="范围类型" rules={[{ required: true }]}>
            <Select options={[{ label: '设备', value: 'device' }, { label: '组', value: 'group' }]} />
          </Form.Item>
          <Form.Item name="template" label="策略模板 (JSON)"><Input.TextArea rows={4} placeholder='{"schedule":"daily"}' /></Form.Item>
        </Form>
      </Modal>
    </>
  );
}

function UserTab() {
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [form] = Form.useForm();

  const fetch = useCallback(async () => {
    setLoading(true);
    try { setUsers((await userApi.list()).users); } catch { setUsers([]); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);

  const onCreate = async () => {
    const values = await form.validateFields();
    await userApi.create(values);
    message.success('用户创建成功');
    setModalOpen(false);
    form.resetFields();
    fetch();
  };

  return (
    <>
      <Space style={{ marginBottom: 16 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => setModalOpen(true)}>添加用户</Button>
        <Button icon={<ReloadOutlined />} onClick={fetch}>刷新</Button>
      </Space>
      <Table<User>
        rowKey="user_id"
        dataSource={users}
        loading={loading}
        columns={[
          { title: '用户名', dataIndex: 'username', key: 'username' },
          { title: '显示名', dataIndex: 'display_name', key: 'display_name' },
          { title: '邮箱', dataIndex: 'email', key: 'email' },
          { title: '来源', dataIndex: 'auth_source', key: 'auth_source' },
          { title: '状态', dataIndex: 'status', key: 'status', render: (s: string) => <Tag color={s === 'active' ? 'success' : 'error'}>{s}</Tag> },
          {
            title: '操作', key: 'action', render: (_, r) => (
              <Popconfirm title="确认删除？" onConfirm={async () => { await userApi.remove(r.user_id); message.success('已删除'); fetch(); }}>
                <Button danger size="small" icon={<DeleteOutlined />}>删除</Button>
              </Popconfirm>
            ),
          },
        ]}
      />
      <Modal title="添加用户" open={modalOpen} onOk={onCreate} onCancel={() => setModalOpen(false)}>
        <Form form={form} layout="vertical">
          <Form.Item name="username" label="用户名" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="display_name" label="显示名" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="email" label="邮箱" rules={[{ required: true, type: 'email' }]}><Input /></Form.Item>
          <Form.Item name="password" label="密码" rules={[{ required: true }]}><Input.Password /></Form.Item>
        </Form>
      </Modal>
    </>
  );
}

function RoleTab() {
  const [roles, setRoles] = useState<Role[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [form] = Form.useForm();

  const fetch = useCallback(async () => {
    setLoading(true);
    try { setRoles((await roleApi.list()).roles); } catch { setRoles([]); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);

  const onCreate = async () => {
    const values = await form.validateFields();
    await roleApi.create(values);
    message.success('角色创建成功');
    setModalOpen(false);
    form.resetFields();
    fetch();
  };

  return (
    <>
      <Space style={{ marginBottom: 16 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => setModalOpen(true)}>添加角色</Button>
        <Button icon={<ReloadOutlined />} onClick={fetch}>刷新</Button>
      </Space>
      <Table<Role>
        rowKey="role_id"
        dataSource={roles}
        loading={loading}
        columns={[
          { title: '名称', dataIndex: 'name', key: 'name' },
          { title: '内置', dataIndex: 'is_builtin', key: 'is_builtin', render: (v: boolean) => v ? <Tag color="blue">内置</Tag> : <Tag>自定义</Tag> },
          { title: '权限', dataIndex: 'permissions', key: 'permissions', render: (p: string[]) => p?.map((perm) => <Tag key={perm}>{perm}</Tag>) },
        ]}
      />
      <Modal title="添加角色" open={modalOpen} onOk={onCreate} onCancel={() => setModalOpen(false)}>
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="名称" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="permissions" label="权限（逗号分隔）" rules={[{ required: true }]}>
            <Input placeholder="devices:read,jobs:*,logs:read" />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}

function OrgTab() {
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [form] = Form.useForm();

  const fetch = useCallback(async () => {
    setLoading(true);
    try { setOrgs((await orgApi.list()).organizations); } catch { setOrgs([]); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);

  const onCreate = async () => {
    const values = await form.validateFields();
    await orgApi.create(values);
    message.success('组织创建成功');
    setModalOpen(false);
    form.resetFields();
    fetch();
  };

  return (
    <>
      <Space style={{ marginBottom: 16 }}>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => setModalOpen(true)}>添加组织</Button>
        <Button icon={<ReloadOutlined />} onClick={fetch}>刷新</Button>
      </Space>
      <Table<Organization>
        rowKey="organization_id"
        dataSource={orgs}
        loading={loading}
        columns={[
          { title: '名称', dataIndex: 'name', key: 'name' },
          { title: '路径', dataIndex: 'path', key: 'path' },
          { title: '创建时间', dataIndex: 'created_at', key: 'created_at', render: (t: string) => new Date(t).toLocaleString() },
        ]}
      />
      <Modal title="添加组织" open={modalOpen} onOk={onCreate} onCancel={() => setModalOpen(false)}>
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="名称" rules={[{ required: true }]}><Input /></Form.Item>
        </Form>
      </Modal>
    </>
  );
}

function formatBytes(bytes: number): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}