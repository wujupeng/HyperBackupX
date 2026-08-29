import { useEffect, useState } from 'react';
import { Card, Table, Button, Space, Typography, Tag, Statistic, Row, Col, Select } from 'antd';
import { ReloadOutlined, PlayCircleOutlined } from '@ant-design/icons';
import ReactECharts from 'echarts-for-react';

const { Title } = Typography;

interface MatrixEntry {
  layer: string;
  backend: string;
  feature: string;
  status: string;
}

const mockData: MatrixEntry[] = [
  { layer: 'L1', backend: 'Local', feature: 'backup', status: 'PASS' },
  { layer: 'L1', backend: 'S3', feature: 'backup', status: 'PASS' },
  { layer: 'L1', backend: 'FTP', feature: 'backup', status: 'PASS' },
  { layer: 'L2', backend: 'Local', feature: 'incremental', status: 'PASS' },
  { layer: 'L2', backend: 'S3', feature: 'incremental', status: 'PASS' },
  { layer: 'L3', backend: 'Local', feature: 'semantic', status: 'PASS' },
  { layer: 'L3', backend: 'S3', feature: 'semantic', status: 'PASS' },
  { layer: 'L4', backend: 'Local', feature: 'exception', status: 'PASS' },
  { layer: 'L4', backend: 'S3', feature: 'exception', status: 'MISSING' },
  { layer: 'L5', backend: 'Local', feature: 'restore', status: 'PASS' },
  { layer: 'L5', backend: 'S3', feature: 'restore', status: 'PASS' },
];

export default function CompatMatrixPage() {
  const [data] = useState<MatrixEntry[]>(mockData);
  const [layerFilter, setLayerFilter] = useState<string>('');
  const [loading, setLoading] = useState(false);

  useEffect(() => { setLoading(false); }, []);

  const layers = ['L1', 'L2', 'L3', 'L4', 'L5'];
  const backends = ['Local', 'S3', 'FTP'];

  const layerPassRate = (layer: string) => {
    const entries = data.filter(e => e.layer === layer);
    const passed = entries.filter(e => e.status === 'PASS').length;
    return entries.length > 0 ? Math.round((passed / entries.length) * 100) : 0;
  };

  const heatmapData: [number, number, number][] = [];
  layers.forEach((layer, li) => {
    backends.forEach((backend, bi) => {
      const entries = data.filter(e => e.layer === layer && e.backend === backend);
      const passCount = entries.filter(e => e.status === 'PASS').length;
      const rate = entries.length > 0 ? (passCount / entries.length) * 100 : 0;
      heatmapData.push([bi, li, rate]);
    });
  });

  const heatmapOption = {
    tooltip: { formatter: (p: { data: [number, number, number] }) => `${backends[p.data[0]]} / ${layers[p.data[1]]}: ${p.data[2]}%` },
    xAxis: { type: 'category', data: backends },
    yAxis: { type: 'category', data: layers },
    visualMap: { min: 0, max: 100, calculable: true, orient: 'horizontal', left: 'center', bottom: 0, inRange: { color: ['#ff4d4f', '#faad14', '#52c41a'] } },
    series: [{ type: 'heatmap', data: heatmapData, label: { show: true, formatter: (p: { data: [number, number, number] }) => `${p.data[2]}%` } }],
  };

  const filtered = layerFilter ? data.filter(e => e.layer === layerFilter) : data;
  const statusColor: Record<string, string> = { PASS: 'green', FAIL: 'red', MISSING: 'orange', NOT_APPLICABLE: 'default' };

  const columns = [
    { title: '层级', dataIndex: 'layer', key: 'layer', render: (v: string) => <Tag color="blue">{v}</Tag> },
    { title: '后端', dataIndex: 'backend', key: 'backend' },
    { title: '功能项', dataIndex: 'feature', key: 'feature' },
    { title: '状态', dataIndex: 'status', key: 'status', render: (v: string) => <Tag color={statusColor[v] || 'default'}>{v}</Tag> },
  ];

  return (
    <div>
      <Title level={2}>兼容性矩阵看板</Title>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        {layers.map(layer => (
          <Col key={layer} span={4}>
            <Card>
              <Statistic title={layer} value={layerPassRate(layer)} suffix="%" />
            </Card>
          </Col>
        ))}
      </Row>

      <Card title="矩阵热力图" style={{ marginBottom: 16 }}>
        <ReactECharts option={heatmapOption} style={{ height: 350 }} />
      </Card>

      <Card>
        <Space style={{ marginBottom: 16 }}>
          <Select placeholder="按层级过滤" allowClear style={{ width: 120 }} onChange={setLayerFilter} options={layers.map(l => ({ label: l, value: l }))} />
          <Button icon={<ReloadOutlined />} onClick={() => setLoading(false)}>刷新</Button>
          <Button type="primary" icon={<PlayCircleOutlined />}>执行矩阵</Button>
        </Space>
        <Table<MatrixEntry> rowKey={(r) => `${r.layer}-${r.backend}-${r.feature}`} dataSource={filtered} columns={columns} loading={loading} />
      </Card>
    </div>
  );
}