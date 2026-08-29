import { Input, InputNumber, Switch, Form } from 'antd';
import type { FormInstance } from 'antd';
import type { BackendType } from '../../api/types';
import { getBackendFields } from './backendFields';

interface RepoConnectionFormProps {
  backendType: BackendType;
  form: FormInstance;
  initialConfig?: Record<string, unknown>;
  isEdit?: boolean;
}

export default function RepoConnectionForm({ backendType, initialConfig, isEdit }: RepoConnectionFormProps) {
  const fields = getBackendFields(backendType);

  return (
    <>
      {fields.map((field) => {
        const initialValue = isEdit && field.sensitive
          ? ''
          : initialConfig?.[field.name] ?? field.placeholder ?? '';

        const rules: Record<string, unknown>[] = [];
        if (field.required) {
          rules.push({ required: true, message: `请输入${field.label}` });
        }
        if (field.type === 'number') {
          rules.push({ type: 'number', max: 65535, message: '端口范围 1-65535' });
        }
        if (field.name === 'url' || field.name === 'endpoint_url' || field.name === 'auth_url') {
          rules.push({ type: 'url', message: '请输入有效的 URL' });
        }

        return (
          <Form.Item
            key={field.name}
            name={['connection_config', field.name]}
            label={field.label}
            rules={rules}
            initialValue={field.type === 'switch' ? !!initialValue : initialValue}
          >
            {field.type === 'text' && (field.sensitive
              ? <Input.Password placeholder={field.placeholder} />
              : <Input placeholder={field.placeholder} />
            )}
            {field.type === 'number' && <InputNumber min={1} max={65535} style={{ width: '100%' }} placeholder={field.placeholder} />}
            {field.type === 'switch' && <Switch />}
          </Form.Item>
        );
      })}
    </>
  );
}