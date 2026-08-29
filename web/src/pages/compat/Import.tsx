import { useState } from 'react';
import { Card, Button, Space, Typography, Steps, Upload, Alert, Table, Tag, message, Result } from 'antd';
import { UploadOutlined, ImportOutlined, FileTextOutlined } from '@ant-design/icons';
import { compatImportApi, type ImportResult, type FieldMapping, type UnsupportedItem } from '../../api/compat';

const { Title } = Typography;

export default function CompatImportPage() {
  const [current, setCurrent] = useState(0);
  const [configData, setConfigData] = useState('');
  const [configName, setConfigName] = useState('');
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [loading, setLoading] = useState(false);

  const handleUpload = (file: File) => {
    const reader = new FileReader();
    reader.onload = (e) => {
      const text = e.target?.result as string;
      setConfigData(text);
      setConfigName(file.name);
      setCurrent(1);
    };
    reader.readAsText(file);
    return false;
  };

  const handleImport = async () => {
    setLoading(true);
    try {
      const result = await compatImportApi.import({ format: 'json', config: configData });
      setImportResult(result);
      setCurrent(2);
      if (result.status === 'success') {
        message.success('导入成功');
      } else if (result.status === 'partial') {
        message.warning('部分导入成功，存在不支持项');
      }
    } catch {
      message.error('导入失败');
    } finally {
      setLoading(false);
    }
  };

  const handleReset = () => {
    setCurrent(0);
    setConfigData('');
    setConfigName('');
    setImportResult(null);
  };

  const mappingColumns = [
    { title: 'Duplicati 字段', dataIndex: 'duplicati_field', key: 'duplicati_field' },
    { title: 'HBX 字段', dataIndex: 'hbx_field', key: 'hbx_field' },
    { title: '支持', dataIndex: 'supported', key: 'supported', render: (v: boolean) => <Tag color={v ? 'green' : 'red'}>{v ? '是' : '否'}</Tag> },
  ];

  const unsupportedColumns = [
    { title: '字段', dataIndex: 'field', key: 'field' },
    { title: '原因', dataIndex: 'reason', key: 'reason' },
    { title: '处理', dataIndex: 'action', key: 'action', render: (v: string) => <Tag color={v === 'abort' ? 'red' : 'orange'}>{v}</Tag> },
  ];

  return (
    <div>
      <Title level={2}>Duplicati 配置导入</Title>
      <Card>
        <Steps current={current} items={[
          { title: '上传配置' },
          { title: '预览确认' },
          { title: '导入结果' },
        ]} />

        <div style={{ marginTop: 24 }}>
          {current === 0 && (
            <Upload.Dragger accept=".json" beforeUpload={handleUpload} maxCount={1}>
              <p className="ant-upload-drag-icon"><UploadOutlined /></p>
              <p className="ant-upload-text">点击或拖拽 Duplicati 配置文件到此区域</p>
              <p className="ant-upload-hint">支持 JSON 格式</p>
            </Upload.Dragger>
          )}

          {current === 1 && (
            <div>
              <Alert message={`已加载配置文件: ${configName}`} type="info" showIcon style={{ marginBottom: 16 }} />
              <Space>
                <Button icon={<FileTextOutlined />} onClick={() => message.info('预览功能请使用 dry-run CLI')}>预览映射</Button>
                <Button type="primary" icon={<ImportOutlined />} loading={loading} onClick={handleImport}>确认导入</Button>
                <Button onClick={handleReset}>取消</Button>
              </Space>
            </div>
          )}

          {current === 2 && importResult && (
            <div>
              {importResult.status === 'success' && (
                <Result status="success" title="导入成功" subTitle={`导入ID: ${importResult.import_id}`} />
              )}
              {importResult.status === 'partial' && (
                <Result status="warning" title="部分导入成功" subTitle="存在不支持项，请查看详情" />
              )}
              {importResult.status === 'failed' && (
                <Result status="error" title="导入失败" subTitle="请检查配置格式" />
              )}

              {importResult.idempotent && (
                <Alert message="幂等命中：该配置已导入过，返回已有结果" type="info" showIcon style={{ marginBottom: 16 }} />
              )}

              {importResult.field_mappings.length > 0 && (
                <Card title="字段映射" size="small" style={{ marginBottom: 16 }}>
                  <Table<FieldMapping> rowKey="duplicati_field" dataSource={importResult.field_mappings} columns={mappingColumns} pagination={false} size="small" />
                </Card>
              )}

              {importResult.unsupported_items.length > 0 && (
                <Card title="不支持项" size="small">
                  <Table<UnsupportedItem> rowKey="field" dataSource={importResult.unsupported_items} columns={unsupportedColumns} pagination={false} size="small" />
                </Card>
              )}

              <Button style={{ marginTop: 16 }} onClick={handleReset}>再次导入</Button>
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}