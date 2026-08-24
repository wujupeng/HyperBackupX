import { useState } from 'react';
import {
  Steps, Form, Input, Select, Button, Space, Card, Typography,
  InputNumber, Switch, Radio, DatePicker, Alert, message,
} from 'antd';
import { useNavigate } from 'react-router-dom';
import { repositoryApi, jobApi, deviceApi } from '../api/endpoints';

const { Title } = Typography;
const { RangePicker } = DatePicker;

type FormData = {
  sourcePath: string;
  includeRules: string;
  excludeRules: string;
  repositoryId: string;
  scheduleType: string;
  scheduleInterval: number;
  scheduleTime: string;
  retentionMode: string;
  keepLastN: number;
  encryptionEnabled: boolean;
  encryptionPassword: string;
  encryptionKeySource: string;
  jobName: string;
};

export default function BackupWizard() {
  const navigate = useNavigate();
  const [current, setCurrent] = useState(0);
  const [form] = Form.useForm<FormData>();
  const [submitting, setSubmitting] = useState(false);
  const [repos, setRepos] = useState<{ label: string; value: string }[]>([]);
  const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);

  const loadOptions = async () => {
    try {
      const repoRes = await repositoryApi.list();
      setRepos(repoRes.repositories.map((r) => ({ label: r.name, value: r.repository_id })));
      const devRes = await deviceApi.list();
      setDevices(devRes.devices.map((d) => ({ label: d.hostname, value: d.device_id })));
    } catch {
      // API may be unavailable
    }
  };

  useState(() => { loadOptions(); });

  const steps = [
    { title: '源选择' },
    { title: '目标仓库' },
    { title: '调度' },
    { title: '保留策略' },
    { title: '加密' },
    { title: '确认' },
  ];

  const next = async () => {
    try {
      const fields = ['sourcePath', 'repositoryId', 'scheduleType', 'retentionMode', 'encryptionEnabled', 'jobName'] as const;
      await form.validateFields([fields[current]]);
      setCurrent(current + 1);
    } catch {
      // validation failed
    }
  };

  const prev = () => setCurrent(current - 1);

  const onFinish = async () => {
    setSubmitting(true);
    try {
      const values = form.getFieldsValue();
      await jobApi.create({
        name: values.jobName,
        device_id: devices[0]?.value,
        source_config: {
          paths: [values.sourcePath],
          include_rules: values.includeRules?.split(',').map((s) => s.trim()).filter(Boolean) || [],
          exclude_rules: values.excludeRules?.split(',').map((s) => s.trim()).filter(Boolean) || [],
        },
        destination_config: {
          repository_id: values.repositoryId,
        },
        schedule: {
          type: values.scheduleType,
          interval: values.scheduleInterval,
          time: values.scheduleTime,
        },
        retention: {
          mode: values.retentionMode,
          keep_last_n: values.keepLastN,
        },
        encryption: {
          enabled: values.encryptionEnabled,
          key_source: values.encryptionKeySource,
        },
      });
      message.success('备份任务创建成功');
      navigate('/jobs');
    } catch {
      message.error('创建失败，请检查输入');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div>
      <Title level={2}>创建备份任务</Title>
      <Card>
        <Steps current={current} items={steps} style={{ marginBottom: 32 }} />
        <Form form={form} layout="vertical">
          {current === 0 && (
            <>
              <Form.Item name="jobName" label="任务名称" rules={[{ required: true, message: '请输入任务名称' }]}>
                <Input placeholder="例如：每日文档备份" />
              </Form.Item>
              <Form.Item name="sourcePath" label="源路径" rules={[{ required: true, message: '请输入源路径' }]}>
                <Input placeholder="例如：C:\Users\Documents 或 /home/user/data" />
              </Form.Item>
              <Form.Item name="includeRules" label="包含规则（逗号分隔）">
                <Input placeholder="例如：*.docx,*.xlsx,*.pdf" />
              </Form.Item>
              <Form.Item name="excludeRules" label="排除规则（逗号分隔）">
                <Input placeholder="例如：*.tmp,*.log,~*" />
              </Form.Item>
            </>
          )}
          {current === 1 && (
            <Form.Item name="repositoryId" label="目标仓库" rules={[{ required: true, message: '请选择仓库' }]}>
              <Select options={repos} placeholder="选择备份仓库" />
            </Form.Item>
          )}
          {current === 2 && (
            <>
              <Form.Item name="scheduleType" label="调度类型" rules={[{ required: true }]} initialValue="manual">
                <Radio.Group>
                  <Radio value="manual">手动</Radio>
                  <Radio value="interval">间隔</Radio>
                  <Radio value="daily">每日</Radio>
                  <Radio value="weekly">每周</Radio>
                  <Radio value="monthly">每月</Radio>
                  <Radio value="cron">Cron 表达式</Radio>
                </Radio.Group>
              </Form.Item>
              <Form.Item shouldUpdate noStyle>
                {() => form.getFieldValue('scheduleType') === 'interval' && (
                  <Form.Item name="scheduleInterval" label="间隔（分钟）" initialValue={60}>
                    <InputNumber min={1} />
                  </Form.Item>
                )}
              </Form.Item>
              <Form.Item shouldUpdate noStyle>
                {() => ['daily', 'weekly', 'monthly'].includes(form.getFieldValue('scheduleType')) && (
                  <Form.Item name="scheduleTime" label="执行时间" initialValue="02:00">
                    <Input placeholder="HH:MM" />
                  </Form.Item>
                )}
              </Form.Item>
            </>
          )}
          {current === 3 && (
            <>
              <Form.Item name="retentionMode" label="保留策略" rules={[{ required: true }]} initialValue="keep_last_n">
                <Radio.Group>
                  <Radio value="keep_all">保留全部</Radio>
                  <Radio value="keep_last_n">保留最近 N 个</Radio>
                  <Radio value="time_based">按时间</Radio>
                  <Radio value="gfs">GFS（祖父-父亲-儿子）</Radio>
                  <Radio value="smart">智能</Radio>
                </Radio.Group>
              </Form.Item>
              <Form.Item shouldUpdate noStyle>
                {() => form.getFieldValue('retentionMode') === 'keep_last_n' && (
                  <Form.Item name="keepLastN" label="保留版本数" initialValue={7}>
                    <InputNumber min={1} />
                  </Form.Item>
                )}
              </Form.Item>
              <Form.Item shouldUpdate noStyle>
                {() => form.getFieldValue('retentionMode') === 'time_based' && (
                  <Form.Item name="retentionRange" label="保留时间范围">
                    <RangePicker />
                  </Form.Item>
                )}
              </Form.Item>
            </>
          )}
          {current === 4 && (
            <>
              <Form.Item name="encryptionEnabled" label="启用加密" valuePropName="checked" initialValue={false}>
                <Switch />
              </Form.Item>
              <Form.Item shouldUpdate noStyle>
                {() => form.getFieldValue('encryptionEnabled') && (
                  <>
                    <Form.Item name="encryptionKeySource" label="密钥来源" initialValue="password">
                      <Select
                        options={[
                          { label: '口令派生', value: 'password' },
                          { label: '密钥文件', value: 'keyfile' },
                          { label: 'KMS', value: 'kms' },
                        ]}
                      />
                    </Form.Item>
                    <Form.Item shouldUpdate noStyle>
                      {() => form.getFieldValue('encryptionKeySource') === 'password' && (
                        <Form.Item name="encryptionPassword" label="加密口令" rules={[{ required: true, message: '请输入口令' }]}>
                          <Input.Password />
                        </Form.Item>
                      )}
                    </Form.Item>
                  </>
                )}
              </Form.Item>
            </>
          )}
          {current === 5 && (
            <Form.Item>
              <Alert
                type="info"
                message="请确认以下配置"
                description={
                  <pre style={{ fontSize: 12, margin: 0 }}>
                    {JSON.stringify(form.getFieldsValue(), null, 2)}
                  </pre>
                }
              />
            </Form.Item>
          )}
        </Form>

        <div style={{ marginTop: 24, display: 'flex', justifyContent: 'space-between' }}>
          <Button disabled={current === 0} onClick={prev}>
            上一步
          </Button>
          <Space>
            {current < steps.length - 1 && (
              <Button type="primary" onClick={next}>
                下一步
              </Button>
            )}
            {current === steps.length - 1 && (
              <Button type="primary" loading={submitting} onClick={onFinish}>
                创建任务
              </Button>
            )}
          </Space>
        </div>
      </Card>
    </div>
  );
}