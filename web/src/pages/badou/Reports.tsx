import { useState, useEffect } from 'react';
import { Button, Space, Select, message, Typography, Tag, Card, Descriptions, Empty } from 'antd';
import { ReloadOutlined, SafetyOutlined, ExperimentOutlined } from '@ant-design/icons';
import { badouRepoApi } from '../../api/endpoints';
import type { BadouRepository, BadouGCReport, BadouVerifyResult } from '../../api/types';
import { formatBytes, formatTimestamp } from '../../common/format';

const { Title } = Typography;

export default function BadouReportsPage() {
  const [repos, setRepos] = useState<BadouRepository[]>([]);
  const [selectedRepo, setSelectedRepo] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [verifyResult, setVerifyResult] = useState<BadouVerifyResult | null>(null);
  const [gcReport, setGcReport] = useState<BadouGCReport | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [triggeringGC, setTriggeringGC] = useState(false);

  const fetchRepos = async () => {
    setLoading(true);
    try {
      const res = await badouRepoApi.list();
      setRepos(res.repositories || []);
    } catch {
      setRepos([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchRepos(); }, []);

  const handleVerify = async () => {
    if (!selectedRepo) return;
    setVerifying(true);
    setVerifyResult(null);
    try {
      const result = await badouRepoApi.verify(selectedRepo, 'full');
      setVerifyResult(result);
      if (result.passed) {
        message.success('校验通过');
      } else {
        message.error('校验未通过');
      }
    } catch {
      message.error('校验失败');
    } finally {
      setVerifying(false);
    }
  };

  const handleGC = async () => {
    if (!selectedRepo) return;
    setTriggeringGC(true);
    setGcReport(null);
    try {
      const report = await badouRepoApi.triggerGC(selectedRepo);
      setGcReport(report);
      message.success('GC 完成');
    } catch {
      message.error('GC 失败');
    } finally {
      setTriggeringGC(false);
    }
  };

  const handleFetchGCReport = async () => {
    if (!selectedRepo) return;
    try {
      const report = await badouRepoApi.getGCReport(selectedRepo);
      setGcReport(report);
    } catch {
      setGcReport(null);
      message.info('暂无 GC 报告');
    }
  };

  return (
    <div>
      <Title level={2}>八斗校验与 GC 报告</Title>
      <Space style={{ marginBottom: 16 }}>
        <Select
          style={{ width: 300 }}
          placeholder="选择仓库"
          value={selectedRepo || undefined}
          onChange={setSelectedRepo}
          loading={loading}
          options={repos.map(r => ({ value: r.repo_id, label: r.name }))}
        />
        <Button icon={<SafetyOutlined />} loading={verifying} onClick={handleVerify} disabled={!selectedRepo}>触发校验</Button>
        <Button icon={<ExperimentOutlined />} loading={triggeringGC} onClick={handleGC} disabled={!selectedRepo}>触发 GC</Button>
        <Button icon={<ReloadOutlined />} onClick={handleFetchGCReport} disabled={!selectedRepo}>查询 GC 报告</Button>
      </Space>

      {verifyResult && (
        <Card title="校验结果" style={{ marginBottom: 16 }}>
          <Descriptions column={2}>
            <Descriptions.Item label="仓库 ID">{verifyResult.repo_id.slice(0, 12)}</Descriptions.Item>
            <Descriptions.Item label="校验级别">{verifyResult.level}</Descriptions.Item>
            <Descriptions.Item label="结果">
              {verifyResult.passed ? <Tag color="green">PASS</Tag> : <Tag color="red">FAIL</Tag>}
            </Descriptions.Item>
            <Descriptions.Item label="错误数">{verifyResult.errors}</Descriptions.Item>
            <Descriptions.Item label="警告数">{verifyResult.warnings}</Descriptions.Item>
          </Descriptions>
        </Card>
      )}

      {gcReport && (
        <Card title="GC 报告">
          <Descriptions column={2}>
            <Descriptions.Item label="报告 ID">{gcReport.report_id.slice(0, 12)}</Descriptions.Item>
            <Descriptions.Item label="触发者">{gcReport.triggered_by || '-'}</Descriptions.Item>
            <Descriptions.Item label="状态">
              <Tag color={gcReport.status === 'completed' ? 'green' : gcReport.status === 'failed' ? 'red' : 'blue'}>
                {gcReport.status}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item label="耗时">{gcReport.duration_ms} ms</Descriptions.Item>
            <Descriptions.Item label="扫描 Chunk 数">{gcReport.chunks_scanned}</Descriptions.Item>
            <Descriptions.Item label="删除 Chunk 数">{gcReport.chunks_deleted}</Descriptions.Item>
            <Descriptions.Item label="释放空间">{formatBytes(gcReport.bytes_freed)}</Descriptions.Item>
            <Descriptions.Item label="开始时间">{formatTimestamp(gcReport.started_at)}</Descriptions.Item>
            <Descriptions.Item label="完成时间">{formatTimestamp(gcReport.completed_at)}</Descriptions.Item>
          </Descriptions>
        </Card>
      )}

      {!verifyResult && !gcReport && (
        <Empty description="选择仓库后触发校验或 GC 查看报告" />
      )}
    </div>
  );
}