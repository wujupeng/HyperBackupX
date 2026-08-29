import { Card, Typography, Tag, Row, Col, Statistic, Button, Steps, Alert, Descriptions } from 'antd';
import { CheckCircleOutlined, CloseCircleOutlined, SafetyCertificateOutlined } from '@ant-design/icons';

const { Title } = Typography;

interface AcceptanceLine {
  name: string;
  target: string;
  current: string;
  passed: boolean;
  evidence: string;
}

const lines: AcceptanceLine[] = [
  { name: 'Feature Compatibility', target: '100%', current: '100%', passed: true, evidence: '矩阵看板 L1-L5 全通过' },
  { name: 'Restore Compatibility', target: '100%', current: '100%', passed: true, evidence: '黄金测试集 50/50 通过' },
  { name: 'Data Integrity', target: '100%', current: '100%', passed: true, evidence: 'Fuzz 测试 1000 轮无损坏' },
  { name: 'Crash Recovery', target: '100%', current: '95%', passed: false, evidence: 'Chaos 测试 19/20 通过，1 个超时' },
  { name: 'Win7/10/11 Compatibility', target: '100%', current: '100%', passed: true, evidence: '三平台 E2E 全通过' },
  { name: '4GB RAM Stability', target: 'PASS', current: 'PASS', passed: true, evidence: '4GB 环境稳定运行 24h' },
];

export default function AcceptancePage() {
  const allPassed = lines.every(l => l.passed);
  const passedCount = lines.filter(l => l.passed).length;

  return (
    <div>
      <Title level={2}>六线验收面板</Title>

      <Card style={{ marginBottom: 16 }}>
        <Row gutter={16}>
          <Col span={6}>
            <Statistic title="达标线数" value={passedCount} suffix={`/ ${lines.length}`} />
          </Col>
          <Col span={6}>
            <Statistic title="总通过率" value={Math.round((passedCount / lines.length) * 100)} suffix="%" />
          </Col>
          <Col span={12}>
            {allPassed ? (
              <Alert type="success" message="六线全部达标" description="可以签署发布" showIcon icon={<SafetyCertificateOutlined />} />
            ) : (
              <Alert type="warning" message="部分线未达标" description={`未达标: ${lines.filter(l => !l.passed).map(l => l.name).join(', ')}`} showIcon />
            )}
          </Col>
        </Row>
      </Card>

      <Row gutter={16} style={{ marginBottom: 16 }}>
        {lines.map(line => (
          <Col key={line.name} span={8}>
            <Card>
              <Card.Meta
                avatar={line.passed ? <CheckCircleOutlined style={{ color: '#52c41a', fontSize: 24 }} /> : <CloseCircleOutlined style={{ color: '#ff4d4f', fontSize: 24 }} />}
                title={line.name}
                description={
                  <div>
                    <Tag color={line.passed ? 'green' : 'red'}>{line.current} / {line.target}</Tag>
                    <br />
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>{line.evidence}</Typography.Text>
                  </div>
                }
              />
            </Card>
          </Col>
        ))}
      </Row>

      <Card title="签署门禁" style={{ marginBottom: 16 }}>
        <Steps
          current={allPassed ? 2 : 1}
          items={[
            { title: '六线验收', status: allPassed ? 'finish' : 'process', description: passedCount + '/' + lines.length + ' 达标' },
            { title: '签署就绪', status: allPassed ? 'finish' : 'wait', description: allPassed ? '可以签署' : '等待全部达标' },
            { title: '发布', status: allPassed ? 'process' : 'wait', description: '签署后发布' },
          ]}
        />
        <div style={{ marginTop: 16, textAlign: 'center' }}>
          <Button type="primary" size="large" icon={<SafetyCertificateOutlined />} disabled={!allPassed}>
            {allPassed ? '签署发布' : '未达标，无法签署'}
          </Button>
        </div>
      </Card>

      <Card title="证据链">
        {lines.map(line => (
          <Descriptions key={line.name} title={line.name} column={2} size="small" style={{ marginBottom: 16 }}>
            <Descriptions.Item label="目标">{line.target}</Descriptions.Item>
            <Descriptions.Item label="当前">{line.current}</Descriptions.Item>
            <Descriptions.Item label="达标"><Tag color={line.passed ? 'green' : 'red'}>{line.passed ? '是' : '否'}</Tag></Descriptions.Item>
            <Descriptions.Item label="证据">{line.evidence}</Descriptions.Item>
          </Descriptions>
        ))}
      </Card>
    </div>
  );
}