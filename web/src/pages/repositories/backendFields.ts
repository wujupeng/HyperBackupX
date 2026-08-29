import type { BackendType } from '../../api/types';

export interface BackendField {
  name: string;
  label: string;
  required: boolean;
  type: 'text' | 'number' | 'switch';
  sensitive?: boolean;
  placeholder?: string;
}

export function getBackendFields(backendType: BackendType): BackendField[] {
  switch (backendType) {
    case 'local':
      return [
        { name: 'path', label: '路径', required: true, type: 'text', placeholder: '/data/backup' },
      ];
    case 'smb':
      return [
        { name: 'host', label: '主机', required: true, type: 'text', placeholder: '192.168.1.100' },
        { name: 'share', label: '共享名', required: true, type: 'text' },
        { name: 'username', label: '用户名', required: false, type: 'text' },
        { name: 'password', label: '密码', required: false, type: 'text', sensitive: true },
        { name: 'domain', label: '域', required: false, type: 'text' },
      ];
    case 'ftp':
    case 'ftps':
      return [
        { name: 'host', label: '主机', required: true, type: 'text' },
        { name: 'port', label: '端口', required: true, type: 'number', placeholder: '21' },
        { name: 'username', label: '用户名', required: false, type: 'text' },
        { name: 'password', label: '密码', required: false, type: 'text', sensitive: true },
        { name: 'path', label: '路径', required: false, type: 'text' },
        ...(backendType === 'ftps' ? [{ name: 'use_tls', label: '启用 TLS', required: false, type: 'switch' as const }] : []),
      ];
    case 'sftp':
      return [
        { name: 'host', label: '主机', required: true, type: 'text' },
        { name: 'port', label: '端口', required: true, type: 'number', placeholder: '22' },
        { name: 'username', label: '用户名', required: false, type: 'text' },
        { name: 'password', label: '密码', required: false, type: 'text', sensitive: true },
        { name: 'private_key', label: '私钥', required: false, type: 'text', sensitive: true },
        { name: 'path', label: '路径', required: false, type: 'text' },
      ];
    case 'webdav':
      return [
        { name: 'url', label: 'URL', required: true, type: 'text', placeholder: 'https://dav.example.com' },
        { name: 'username', label: '用户名', required: false, type: 'text' },
        { name: 'password', label: '密码', required: false, type: 'text', sensitive: true },
      ];
    case 's3':
      return [
        { name: 'endpoint_url', label: 'Endpoint URL', required: true, type: 'text', placeholder: 'https://s3.amazonaws.com' },
        { name: 'bucket', label: 'Bucket', required: true, type: 'text' },
        { name: 'region', label: 'Region', required: false, type: 'text', placeholder: 'us-east-1' },
        { name: 'access_key', label: 'Access Key', required: true, type: 'text' },
        { name: 'secret_key', label: 'Secret Key', required: true, type: 'text', sensitive: true },
      ];
    case 'azure_blob':
      return [
        { name: 'account_name', label: '账户名', required: true, type: 'text' },
        { name: 'account_key', label: '账户密钥', required: true, type: 'text', sensitive: true },
        { name: 'container', label: '容器', required: true, type: 'text' },
        { name: 'endpoint_url', label: 'Endpoint URL', required: false, type: 'text', placeholder: 'https://<account>.blob.core.windows.net' },
      ];
    case 'gcs':
      return [
        { name: 'project_id', label: 'Project ID', required: true, type: 'text' },
        { name: 'bucket', label: 'Bucket', required: true, type: 'text' },
        { name: 'access_key', label: 'Access Key', required: false, type: 'text' },
        { name: 'secret_key', label: 'Secret Key', required: false, type: 'text', sensitive: true },
      ];
    case 'openstack':
      return [
        { name: 'auth_url', label: 'Auth URL', required: true, type: 'text' },
        { name: 'username', label: '用户名', required: true, type: 'text' },
        { name: 'password', label: '密码', required: true, type: 'text', sensitive: true },
        { name: 'project_id', label: 'Project ID', required: true, type: 'text' },
        { name: 'container', label: '容器', required: true, type: 'text' },
        { name: 'region', label: 'Region', required: false, type: 'text' },
      ];
    default:
      return [];
  }
}