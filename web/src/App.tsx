import { ConfigProvider, Layout, Menu, Button, Typography, Space } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import {
  DashboardOutlined,
  CloudServerOutlined,
  ScheduleOutlined,
  DatabaseOutlined,
  SettingOutlined,
  LogoutOutlined,
  PlusCircleOutlined,
  HistoryOutlined,
  FolderOpenOutlined,
  FileTextOutlined,
  AuditOutlined,
} from '@ant-design/icons';
import { BrowserRouter, Routes, Route, useNavigate, useLocation } from 'react-router-dom';
import { useAuthStore } from './stores/authStore';
import Dashboard from './pages/Dashboard';
import Login from './pages/Login';
import BackupWizard from './pages/BackupWizard';
import RestoreWizard from './pages/RestoreWizard';
import VersionBrowser from './pages/VersionBrowser';
import FileBrowser from './pages/FileBrowser';
import LogsPage from './pages/LogsPage';
import SettingsPage from './pages/SettingsPage';
import AuditPage from './pages/AuditPage';

const { Header, Sider, Content } = Layout;
const { Title } = Typography;

const menuItems = [
  { key: '/', icon: <DashboardOutlined />, label: '仪表盘' },
  { key: '/devices', icon: <CloudServerOutlined />, label: '设备管理' },
  { key: '/jobs', icon: <ScheduleOutlined />, label: '备份任务' },
  { key: '/backup/new', icon: <PlusCircleOutlined />, label: '新建备份' },
  { key: '/restore/new', icon: <HistoryOutlined />, label: '恢复向导' },
  { key: '/versions', icon: <FolderOpenOutlined />, label: '版本浏览' },
  { key: '/repositories', icon: <DatabaseOutlined />, label: '仓库管理' },
  { key: '/logs', icon: <FileTextOutlined />, label: '日志检索' },
  { key: '/audit', icon: <AuditOutlined />, label: '审计日志' },
  { key: '/settings', icon: <SettingOutlined />, label: '系统设置' },
];

function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { isAuthenticated, displayName, logout } = useAuthStore();

  if (!isAuthenticated) {
    return <Login />;
  }

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider collapsible>
        <div style={{ height: 48, margin: 8, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <Title level={4} style={{ color: 'white', margin: 0 }}>
            HBX
          </Title>
        </div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[location.pathname]}
          items={menuItems}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', padding: '0 24px' }}>
          <Space>
            <Typography.Text style={{ color: 'white' }}>
              {displayName || '用户'}
            </Typography.Text>
            <Button
              type="text"
              icon={<LogoutOutlined />}
              style={{ color: 'white' }}
              onClick={() => {
                logout();
                navigate('/login');
              }}
            >
              退出
            </Button>
          </Space>
        </Header>
        <Content style={{ padding: 24, overflow: 'auto' }}>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/backup/new" element={<BackupWizard />} />
            <Route path="/restore/new" element={<RestoreWizard />} />
            <Route path="/versions" element={<VersionBrowser />} />
            <Route path="/versions/:id/files" element={<FileBrowserRoute />} />
            <Route path="/devices" element={<PlaceholderPage title="设备管理" />} />
            <Route path="/jobs" element={<PlaceholderPage title="备份任务" />} />
            <Route path="/repositories" element={<PlaceholderPage title="仓库管理" />} />
            <Route path="/logs" element={<LogsPage />} />
            <Route path="/audit" element={<AuditPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/login" element={<Login />} />
          </Routes>
        </Content>
      </Layout>
    </Layout>
  );
}

function FileBrowserRoute() {
  const location = useLocation();
  const versionId = location.pathname.split('/')[2] || '';
  return <FileBrowser versionId={versionId} />;
}

function PlaceholderPage({ title }: { title: string }) {
  return (
    <div>
      <Title level={2}>{title}</Title>
      <Typography.Text type="secondary">此页面将在后续任务中实现</Typography.Text>
    </div>
  );
}

export default function App() {
  return (
    <ConfigProvider locale={zhCN}>
      <BrowserRouter>
        <AppLayout />
      </BrowserRouter>
    </ConfigProvider>
  );
}
