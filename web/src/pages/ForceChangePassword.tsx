import { useState } from 'react';
import { Card, Form, Input, Button, message, Typography } from 'antd';
import { useAuthStore } from '../stores/authStore';
import { post } from '../api/client';

export default function ForceChangePassword() {
  const { logout } = useAuthStore();
  const [loading, setLoading] = useState(false);

  const onFinish = async (values: { new_password: string; confirm: string }) => {
    if (values.new_password !== values.confirm) {
      message.error('两次输入的密码不一致');
      return;
    }
    setLoading(true);
    try {
      await post('/auth/change-password', {
        old_password: values.old_password,
        new_password: values.new_password,
      });
      message.success('密码修改成功，正在重新加载...');
      setTimeout(() => window.location.reload(), 1500);
    } catch (err: unknown) {
      const error = err as { response?: { data?: { error?: string } } };
      message.error(error.response?.data?.error || '修改失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '100vh' }}>
      <Card title="强制修改密码" style={{ width: 480 }}>
        <Typography.Paragraph type="warning">
          首次登录或管理员重置后，必须修改密码才能继续使用系统。
        </Typography.Paragraph>
        <Form onFinish={onFinish} layout="vertical">
          <Form.Item name="old_password" label="当前密码" rules={[{ required: true }]}>
            <Input.Password />
          </Form.Item>
          <Form.Item name="new_password" label="新密码" rules={[{ required: true, min: 16 }]}>
            <Input.Password placeholder="至少16位，含大小写字母、数字和符号" />
          </Form.Item>
          <Form.Item name="confirm" label="确认新密码" rules={[{ required: true }]}>
            <Input.Password />
          </Form.Item>
          <Form.Item>
            <Button type="primary" htmlType="submit" loading={loading} block>
              修改密码
            </Button>
          </Form.Item>
          <Form.Item>
            <Button onClick={() => { logout(); window.location.href = '/login'; }} block>
              退出登录
            </Button>
          </Form.Item>
        </Form>
      </Card>
    </div>
  );
}