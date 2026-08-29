import { useState, useEffect } from 'react';
import { Table, Button, Space, Modal, Form, Input, Select, Popconfirm, Drawer, Tag, Checkbox, message, Typography } from 'antd';
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { deviceApi, policyApi } from '../api/endpoints';
import type { Device, Policy } from '../api/types';
import { DeviceStatusTag } from '../common/StatusTags';
import { formatTimestamp } from '../common/format';

const { Title } = Typography;

export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [confirmLoading, setConfirmLoading] = useState(false);
  const [form] = Form.useForm();

  const [policyDrawerId, setPolicyDrawerId] = useState<string | null>(null);
  const [allPolicies, setAllPolicies] = useState<Policy[]>([]);
  const [boundPolicies, setBoundPolicies] = useState<Policy[]>([]);
  const [selectedPolicyIds, setSelectedPolicyIds] = useState<string[]>([]);
  const [binding, setBinding] = useState(false);

  const fetchData = async () => {
    setLoading(true);
    try {
      const res = await deviceApi.list();
      setDevices(res.devices);
    } catch {
      setDevices([]);
      message.error('加载设备列表失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const handleCreate = async () => {
    try {
      const values = await form.validateFields();
      setConfirmLoading(true);
      await deviceApi.create(values);
      message.success('注册成功');
      setCreateOpen(false);
      form.resetFields();
      fetchData();
    } catch {
      message.error('注册失败');
    } finally {
      setConfirmLoading(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deviceApi.remove(id);
      message.success('删除成功');
      fetchData();
    } catch {
      message.error('删除失败');
    }
  };

  const openPolicyDrawer = async (deviceId: string) => {
    setPolicyDrawerId(deviceId);
    try {
      const [policiesRes, boundRes] = await Promise.all([
        policyApi.list(),
        deviceApi.getPolicies(deviceId),
      ]);
      setAllPolicies(policiesRes.policies);
      setBoundPolicies(boundRes.policies || []);
      setSelectedPolicyIds((boundRes.policies || []).map(p => p.policy_id));
    } catch {
      message.error('加载策略失败');
    }
  };

  const handleBindPolicies = async () => {
    if (!policyDrawerId) return;
    setBinding(true);
    try {
      await deviceApi.bindPolicies(policyDrawerId, { policy_ids: selectedPolicyIds });
      message.success('策略绑定成功');
      const boundRes = await deviceApi.getPolicies(policyDrawerId);
      setBoundPolicies(boundRes.policies || []);
    } catch {
      message.error('策略绑定失败');
    } finally {
      setBinding(false);
    }
  };

  const columns = [
    { title: '设备ID', dataIndex: 'device_id', key: 'device_id', render: (v: string) => v.slice(0, 8) },
    { title: '主机名', dataIndex: 'hostname', key: 'hostname' },
    { title: '操作系统', dataIndex: 'os_type', key: 'os_type', render: (v: string) => v.charAt(0).toUpperCase() + v.slice(1) },
    { title: 'Agent版本', dataIndex: 'agent_version', key: 'agent_version' },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => <DeviceStatusTag status={v} /> },
    { title: '最后心跳', dataIndex: 'last_heartbeat', key: 'last_heartbeat', render: (v: string) => formatTimestamp(v) },
    { title: '注册时间', dataIndex: 'registered_at', key: 'registered_at', render: (v: string) => formatTimestamp(v) },
    {
      title: '操作', key: 'action', render: (_: unknown, record: Device) => (
        <Space>
          <Button size="small" onClick={() => openPolicyDrawer(record.device_id)}>策略绑定</Button>
          <Popconfirm title="确定删除此设备？" onConfirm={() => handleDelete(record.device_id)}>
            <Button size="small" danger>删除</Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Title level={2}>设备管理</Title>
      <Space style={{ marginBottom: 16 }}>
        <Button icon={<ReloadOutlined />} onClick={fetchData}>刷新</Button>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => setCreateOpen(true)}>注册设备</Button>
      </Space>
      <Table
        columns={columns}
        dataSource={devices}
        rowKey="device_id"
        loading={loading}
        pagination={{ pageSize: 10, showSizeChanger: true, pageSizeOptions: ['10', '20', '50'] }}
        locale={{ emptyText: '暂无设备，请注册新设备' }}
      />
      <Modal
        title="注册设备"
        open={createOpen}
        onOk={handleCreate}
        onCancel={() => { setCreateOpen(false); form.resetFields(); }}
        confirmLoading={confirmLoading}
        width={520}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="hostname" label="主机名" rules={[{ required: true, message: '请输入主机名' }, { max: 255 }]}>
            <Input />
          </Form.Item>
          <Form.Item name="os_type" label="操作系统类型" rules={[{ required: true, message: '请选择操作系统类型' }]}>
            <Select options={[{ value: 'windows', label: 'Windows' }, { value: 'linux', label: 'Linux' }, { value: 'macos', label: 'macOS' }]} />
          </Form.Item>
        </Form>
      </Modal>
      <Drawer
        title="策略绑定"
        open={policyDrawerId !== null}
        onClose={() => setPolicyDrawerId(null)}
        width={720}
        footer={
          <Space style={{ float: 'right' }}>
            <Button onClick={() => setPolicyDrawerId(null)}>取消</Button>
            <Button type="primary" loading={binding} onClick={handleBindPolicies}>保存</Button>
          </Space>
        }
      >
        <div style={{ marginBottom: 16 }}>
          <Typography.Text strong>已绑定策略：</Typography.Text>
          <div style={{ marginTop: 8 }}>
            {boundPolicies.length > 0 ? boundPolicies.map(p => <Tag key={p.policy_id}>{p.name}</Tag>) : <Typography.Text type="secondary">无</Typography.Text>}
          </div>
        </div>
        <Typography.Text strong>选择策略：</Typography.Text>
        <Checkbox.Group
          style={{ display: 'flex', flexDirection: 'column', marginTop: 8 }}
          value={selectedPolicyIds}
          onChange={(values) => setSelectedPolicyIds(values as string[])}
          options={allPolicies.map(p => ({ label: p.name, value: p.policy_id }))}
        />
      </Drawer>
    </div>
  );
}