import { useEffect } from 'react';
import { Card, Col, Row, Statistic, Typography, Alert as AntAlert, Spin } from 'antd';
import {
  DesktopOutlined,
  CloudServerOutlined,
  DatabaseOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import { useDashboardStore } from '../stores/dashboardStore';
import {
  DeviceStatusChart,
  JobStatusChart,
  StorageChart,
  ThroughputChart,
  DurationChart,
} from '../charts/DashboardCharts';

const { Title } = Typography;

export default function Dashboard() {
  const { data, loading, error, fetchDashboard } = useDashboardStore();

  useEffect(() => {
    fetchDashboard();
    const interval = setInterval(fetchDashboard, 30000);
    return () => clearInterval(interval);
  }, [fetchDashboard]);

  if (loading && !data) {
    return <Spin size="large" style={{ display: 'flex', justifyContent: 'center', padding: 100 }} />;
  }

  return (
    <div>
      <Title level={2}>仪表盘</Title>

      {error && (
        <AntAlert
          message="加载失败"
          description={error}
          type="error"
          closable
          style={{ marginBottom: 16 }}
        />
      )}

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="设备总数"
              value={data?.devices.total ?? 0}
              prefix={<DesktopOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="在线设备"
              value={data?.devices.online ?? 0}
              valueStyle={{ color: '#52c41a' }}
              prefix={<DesktopOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="活跃任务"
              value={data?.jobs.active ?? 0}
              prefix={<CloudServerOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="备份版本"
              value={data?.versions.total ?? 0}
              prefix={<DatabaseOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} sm={12} md={6}>
          <Card>
            <Statistic
              title="活跃告警"
              value={data?.active_alerts ?? 0}
              valueStyle={{ color: (data?.active_alerts ?? 0) > 0 ? '#ff4d4f' : '#52c41a' }}
              prefix={<WarningOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} md={12} lg={6}>
          <Card title="设备状态分布">
            <DeviceStatusChart data={data} />
          </Card>
        </Col>
        <Col xs={24} md={12} lg={6}>
          <Card title="任务状态">
            <JobStatusChart data={data} />
          </Card>
        </Col>
        <Col xs={24} md={12} lg={6}>
          <Card title="仓库容量">
            <StorageChart data={data} />
          </Card>
        </Col>
        <Col xs={24} md={12} lg={6}>
          <Card title="活跃告警数">
            <AntAlert
              message={`${data?.active_alerts ?? 0} 条未处理告警`}
              type={(data?.active_alerts ?? 0) > 0 ? 'warning' : 'success'}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={12}>
          <Card title="吞吐量趋势 (24小时)">
            <ThroughputChart />
          </Card>
        </Col>
        <Col xs={24} lg={12}>
          <Card title="任务耗时分布">
            <DurationChart />
          </Card>
        </Col>
      </Row>
    </div>
  );
}