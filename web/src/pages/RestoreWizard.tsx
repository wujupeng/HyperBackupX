import { useState, useEffect } from 'react';
import {
  Steps, Form, Input, Button, Card, Typography,
  Radio, Alert, message, Table, Tree, Spin,
} from 'antd';
import { useNavigate } from 'react-router-dom';
import { versionApi, restoreApi } from '../api/endpoints';
import type { BackupVersion } from '../api/types';

const { Title } = Typography;

type TreeNode = {
  title: string;
  key: string;
  children?: TreeNode[];
};

export default function RestoreWizard() {
  const navigate = useNavigate();
  const [current, setCurrent] = useState(0);
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);
  const [versions, setVersions] = useState<BackupVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedVersion, setSelectedVersion] = useState<string>('');
  const [fileTree, setFileTree] = useState<TreeNode[]>([]);
  const [checkedKeys, setCheckedKeys] = useState<string[]>([]);

  useEffect(() => {
    setLoading(true);
    versionApi.list()
      .then((res) => setVersions(res.versions))
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  const steps = [
    { title: '版本选择' },
    { title: '文件选择' },
    { title: '恢复模式' },
    { title: '确认' },
  ];

  const onVersionSelect = async (versionId: string) => {
    setSelectedVersion(versionId);
    try {
      const res = await versionApi.files(versionId);
      const files = (res as { files: { path: string; name: string }[] }).files || [];
      setFileTree(buildTree(files));
    } catch {
      setFileTree([]);
    }
    setCurrent(1);
  };

  const onFinish = async () => {
    setSubmitting(true);
    try {
      await restoreApi.create({
        source_version_id: selectedVersion,
        file_selection: { type: 'tree', keys: checkedKeys },
        restore_mode: form.getFieldValue('restoreMode'),
        target_location: form.getFieldValue('targetLocation'),
      });
      message.success('恢复任务已创建');
      navigate('/jobs');
    } catch {
      message.error('创建恢复任务失败');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div>
      <Title level={2}>恢复向导</Title>
      <Card>
        <Steps current={current} items={steps} style={{ marginBottom: 32 }} />

        {current === 0 && (
          <Spin spinning={loading}>
            <Table<BackupVersion>
              rowKey="version_id"
              dataSource={versions}
              pagination={{ pageSize: 10 }}
              rowSelection={{
                type: 'radio',
                onSelect: (record) => onVersionSelect(record.version_id),
              }}
              columns={[
                { title: '版本号', dataIndex: 'version_number', key: 'version_number' },
                { title: '时间', dataIndex: 'timestamp', key: 'timestamp', render: (t: string) => new Date(t).toLocaleString() },
                { title: '类型', dataIndex: 'backup_type', key: 'backup_type' },
                { title: '状态', dataIndex: 'status', key: 'status' },
                { title: '文件数', dataIndex: 'file_count', key: 'file_count' },
                { title: '总大小', dataIndex: 'total_size', key: 'total_size', render: (s: number) => formatBytes(s) },
              ]}
            />
          </Spin>
        )}

        {current === 1 && (
          <>
            {fileTree.length > 0 ? (
              <Tree
                checkable
                treeData={fileTree}
                onCheck={(keys) => setCheckedKeys(keys as string[])}
                defaultExpandAll
              />
            ) : (
              <Alert message="此版本暂无文件信息，将恢复全部文件" type="info" />
            )}
            <div style={{ marginTop: 24, display: 'flex', justifyContent: 'space-between' }}>
              <Button onClick={() => setCurrent(0)}>上一步</Button>
              <Button type="primary" onClick={() => setCurrent(2)}>下一步</Button>
            </div>
          </>
        )}

        {current === 2 && (
          <Form form={form} layout="vertical">
            <Form.Item name="restoreMode" label="恢复模式" rules={[{ required: true }]} initialValue="overwrite">
              <Radio.Group>
                <Radio value="overwrite">覆盖</Radio>
                <Radio value="skip">跳过已存在</Radio>
                <Radio value="rename">重命名（.restored 后缀）</Radio>
                <Radio value="new_location">恢复到新位置</Radio>
              </Radio.Group>
            </Form.Item>
            <Form.Item name="targetLocation" label="目标位置" rules={[{ required: true, message: '请输入目标路径' }]}>
              <Input placeholder="例如：C:\Restored 或 /tmp/restored" />
            </Form.Item>
            <div style={{ marginTop: 24, display: 'flex', justifyContent: 'space-between' }}>
              <Button onClick={() => setCurrent(1)}>上一步</Button>
              <Button type="primary" onClick={() => setCurrent(3)}>下一步</Button>
            </div>
          </Form>
        )}

        {current === 3 && (
          <>
            <Alert
              type="info"
              message="请确认恢复配置"
              description={
                <div>
                  <p>源版本：{selectedVersion}</p>
                  <p>恢复模式：{form.getFieldValue('restoreMode')}</p>
                  <p>目标位置：{form.getFieldValue('targetLocation')}</p>
                  <p>选中文件：{checkedKeys.length} 个</p>
                </div>
              }
            />
            <div style={{ marginTop: 24, display: 'flex', justifyContent: 'space-between' }}>
              <Button onClick={() => setCurrent(2)}>上一步</Button>
              <Button type="primary" loading={submitting} onClick={onFinish}>
                开始恢复
              </Button>
            </div>
          </>
        )}
      </Card>
    </div>
  );
}

function buildTree(files: { path: string; name: string }[]): TreeNode[] {
  const root: TreeNode = { title: '/', key: '/', children: [] };
  for (const file of files) {
    const parts = file.path.split('/').filter(Boolean);
    let current = root;
    for (let i = 0; i < parts.length; i++) {
      const key = '/' + parts.slice(0, i + 1).join('/');
      let child = current.children?.find((c) => c.key === key);
      if (!child) {
        child = { title: parts[i], key, children: [] };
        current.children?.push(child);
      }
      current = child;
    }
  }
  return root.children || [];
}

function formatBytes(bytes: number): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}