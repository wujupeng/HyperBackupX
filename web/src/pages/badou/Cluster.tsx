import { useState, useEffect } from 'react';
import { Table, Button, Space, Modal, Form, Input, InputNumber, Popconfirm, message, Typography, Tag, Card, Statistic, Row, Col, Progress } from 'antd';
import { PlusOutlined, ReloadOutlined, DeleteOutlined, HddOutlined } from '@ant-design/icons';
import { badouClusterApi } from '../../api/endpoints';
import type { BadouNode, BadouClusterHealth } from '../../api/types';
import { formatBytes, formatTimestamp } from '../../common/format';

const { Title } = Typography;

function nodeStatusTag(status: string) {
  const colors: Record<string, string> = { online: 'green', offline: 'default', draining: 'orange', failed: 'red' };
  return <Tag color={colors[status] || 'default'}>{status}</Tag>;
}

function roleTag(role: string) {
  const colors: Record<string, string> = { leader: 'gold', follower: 'blue', learner: 'default' };
  return <Tag color={colors[role] || 'default'}>{role}</Tag>;
}

export default function BadouClusterPage() {
  const [nodes, setNodes] = useState<BadouNode[]>([]);
  const [health, setHealth] = useState<BadouClusterHealth | null>(null);
  const [loading, setLoading] = useState(false);
  const [addModalOpen, setAddModalOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [form] = Form.useForm();
  const [capacityModalOpen, setCapacityModalOpen] = useState(false);
  const [capacityNode, setCapacityNode] = useState<BadouNode | null>(null);
  const [capacityBytes, setCapacityBytes] = useState(0);

  const fetchData = async () => {
    setLoading(true);
    try {
      const [nodesRes, healthRes] = await Promise.allSettled([
        badouClusterApi.listNodes(),
        badouClusterApi.health(),
      ]);
      if (nodesRes.status === 'fulfilled') setNodes(nodesRes.value.nodes || []);
      if (healthRes.status === 'fulfilled') setHealth(healthRes.value);
    } catch {
      message.error('加载集群信息失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchData(); }, []);

  const handleAddNode = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      await badouClusterApi.addNode(values);
      message.success('节点添加成功');
      setAddModalOpen(false);
      form.resetFields();
      fetchData();
    } catch {
      message.error('添加节点失败');
    } finally {
      setSubmitting(false);
    }
  };

  const handleRemoveNode = async (id: string) => {
    try {
      await badouClusterApi.removeNode(id);
      message.success('节点移除成功');
      fetchData();
    } catch {
      message.error('移除节点失败');
    }
  };

  const handleExpandCapacity = async () => {
    if (!capacityNode) return;
    try {
      setSubmitting(true);
      await badouClusterApi.expandCapacity({ node_id: capacityNode.node_id, additional_bytes: capacityBytes });
      message.success('扩容请求已发送');
      setCapacityModalOpen(false);
      fetchData();
    } catch {
      message.error('扩容失败');
    } finally {
      setSubmitting(false);
    }
  };

  const columns = [
    { title: 'ID', dataIndex: 'node_id', key: 'node_id', render: (v: string) => v.slice(0, 8) },
    { title: '地址', key: 'addr', render: (_: unknown, r: BadouNode) => `${r.node_address}:${r.node_port}` },
    { title: '角色', dataIndex: 'node_role', key: 'node_role', render: (v: string) => roleTag(v) },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => nodeStatusTag(v) },
    {
      title: '磁盘用量', key: 'disk', render: (_: unknown, r: BadouNode) => {
        const pct = r.disk_capacity_bytes > 0 ? (r.disk_used_bytes / r.disk_capacity_bytes) * 100 : 0;
        return (
          <Space>
            <Progress percent={Math.round(pct)} size="small" style={{ width: 100 }} />
            <Typography.Text type="secondary">{formatBytes(r.disk_used_bytes)} / {formatBytes(r.disk_capacity_bytes)}</Typography.Text>
          </Space>
        );
      },
    },
    { title: '加入时间', dataIndex: 'joined_at', key: 'joined_at', render: (v: string) => formatTimestamp(v) },
    { title: '最后心跳', dataIndex: 'last_heartbeat_at', key: 'heartbeat', render: (v: string | null) => formatTimestamp(v) },
    {
      title: '操作', key: 'action', render: (_: unknown, r: BadouNode) => (
        <Space>
          <Button size="small" icon={<HddOutlined />} onClick={() => { setCapacityNode(r); setCapacityBytes(0); setCapacityModalOpen(true); }}>扩容</Button>
          <Popconfirm title="确定移除此节点？" onConfirm={() => handleRemoveNode(r.node_id)}>
            <Button size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <Title level={2}>八斗集群健康</Title>

      {health && (
        <Row gutter={16} style={{ marginBottom: 16 }}>
          <Col span={6}>
            <Card>
              <Statistic
                title="集群状态"
                value={health.status}
                valueStyle={{ color: health.status === 'healthy' ? '#3f8600' : '#cf1322' }}
              />
            </Card>
          </Col>
          <Col span={6}>
            <Card><Statistic title="总节点数" value={health.total_nodes} /></Card>
          </Col>
          <Col span={6}>
            <Card><Statistic title="在线节点" value={health.online_nodes} valueStyle={{ color: '#3f8600' }} /></Card>
          </Col>
          <Col span={6}>
            <Card><Statistic title="Leader" value={health.leader_id ? health.leader_id.slice(0, 8) : '-'} /></Card>
          </Col>
        </Row>
      )}

      <Space style={{ marginBottom: 16 }}>
        <Button icon={<ReloadOutlined />} onClick={fetchData} loading={loading}>刷新</Button>
        <Button type="primary" icon={<PlusOutlined />} onClick={() => { form.resetFields(); setAddModalOpen(true); }}>添加节点</Button>
      </Space>

      <Table
        columns={columns}
        dataSource={nodes}
        rowKey="node_id"
        loading={loading}
        pagination={{ pageSize: 10 }}
        locale={{ emptyText: '暂无集群节点' }}
      />

      <Modal title="添加节点" open={addModalOpen} onOk={handleAddNode} onCancel={() => setAddModalOpen(false)} confirmLoading={submitting}>
        <Form form={form} layout="vertical">
          <Form.Item name="node_address" label="节点地址" rules={[{ required: true }]}>
            <Input placeholder="192.168.1.61" />
          </Form.Item>
          <Form.Item name="node_port" label="节点端口" initialValue={50051}>
            <InputNumber min={1} max={65535} />
          </Form.Item>
          <Form.Item name="node_role" label="角色" initialValue="follower">
            <Input placeholder="leader / follower / learner" />
          </Form.Item>
          <Form.Item name="disk_capacity_bytes" label="磁盘容量 (bytes)" initialValue={0}>
            <InputNumber min={0} />
          </Form.Item>
        </Form>
      </Modal>

      <Modal title="扩容磁盘" open={capacityModalOpen} onOk={handleExpandCapacity} onCancel={() => setCapacityModalOpen(false)} confirmLoading={submitting}>
        <Space direction="vertical" style={{ width: '100%' }}>
          <Typography.Text>节点: {capacityNode?.node_address}:{capacityNode?.node_port}</Typography.Text>
          <Typography.Text>当前容量: {formatBytes(capacityNode?.disk_capacity_bytes)}</Typography.Text>
          <InputNumber value={capacityBytes} onChange={(v) => setCapacityBytes(v || 0)} min={0} style={{ width: '100%' }} addonAfter="bytes" />
          <Typography.Text type="secondary">新增 {formatBytes(capacityBytes)} 磁盘空间</Typography.Text>
        </Space>
      </Modal>
    </div>
  );
}