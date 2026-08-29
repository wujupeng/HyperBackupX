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
  ApartmentOutlined,
  ImportOutlined,
  SafetyCertificateOutlined,
  SwapOutlined,
  ClusterOutlined,
} from '@ant-design/icons';
import { BrowserRouter, Routes, Route, useNavigate, useLocation } from 'react-router-dom';
import { useAuthStore } from './stores/authStore';
import Dashboard from './pages/Dashboard';
import Login from './pages/Login';
import ForceChangePassword from './pages/ForceChangePassword';
import BackupWizard from './pages/BackupWizard';
import RestoreWizard from './pages/RestoreWizard';
import VersionBrowser from './pages/VersionBrowser';
import FileBrowser from './pages/FileBrowser';
import LogsPage from './pages/LogsPage';
import SettingsPage from './pages/SettingsPage';
import AuditPage from './pages/AuditPage';
import DevicesPage from './pages/DevicesPage';
import JobsPage from './pages/JobsPage';
import RepositoriesPage from './pages/RepositoriesPage';
import CompatJobsPage from './pages/compat/Jobs';
import CompatImportPage from './pages/compat/Import';
import CompatMatrixPage from './pages/compat/Matrix';
import DualRunPage from './pages/compat/DualRun';
import AcceptancePage from './pages/compat/Acceptance';
import BadouRepositoriesPage from './pages/badou/Repositories';
import BadouReportsPage from './pages/badou/Reports';
import BadouClusterPage from './pages/badou/Cluster';

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
  { key: '/compat/jobs', icon: <ApartmentOutlined />, label: '兼容任务' },
  { key: '/compat/import', icon: <ImportOutlined />, label: '配置导入' },
  { key: '/compat/matrix', icon: <ScheduleOutlined />, label: '兼容矩阵' },
  { key: '/compat/dual-run', icon: <SwapOutlined />, label: '双跑报告' },
  { key: '/compat/acceptance', icon: <SafetyCertificateOutlined />, label: '六线验收' },
  { key: '/badou/repositories', icon: <DatabaseOutlined />, label: '八斗仓库' },
  { key: '/badou/reports', icon: <SafetyCertificateOutlined />, label: '八斗校验/GC' },
  { key: '/badou/cluster', icon: <ClusterOutlined />, label: '八斗集群' },
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
            <Route path="/devices" element={<DevicesPage />} />
            <Route path="/jobs" element={<JobsPage />} />
            <Route path="/repositories" element={<RepositoriesPage />} />
            <Route path="/logs" element={<LogsPage />} />
            <Route path="/audit" element={<AuditPage />} />
            <Route path="/compat/jobs" element={<CompatJobsPage />} />
            <Route path="/compat/import" element={<CompatImportPage />} />
            <Route path="/compat/matrix" element={<CompatMatrixPage />} />
            <Route path="/compat/dual-run" element={<DualRunPage />} />
            <Route path="/compat/acceptance" element={<AcceptancePage />} />
            <Route path="/badou/repositories" element={<BadouRepositoriesPage />} />
            <Route path="/badou/reports" element={<BadouReportsPage />} />
            <Route path="/badou/cluster" element={<BadouClusterPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/login" element={<Login />} />
            <Route path="/force-change-password" element={<ForceChangePassword />} />
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


export default function App() {
  return (
    <ConfigProvider locale={zhCN}>
      <BrowserRouter>
        <AppLayout />
      </BrowserRouter>
    </ConfigProvider>
  );
}
