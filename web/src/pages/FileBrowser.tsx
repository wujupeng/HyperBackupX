import { useState, useEffect } from 'react';
import { Card, Tree, Input, Space, Button, Typography, Spin, Empty, Tag } from 'antd';
import { SearchOutlined, ReloadOutlined } from '@ant-design/icons';
import { versionApi } from '../api/endpoints';

const { Title } = Typography;

type FileNode = {
  path: string;
  name: string;
  size?: number;
  is_dir?: boolean;
};

type TreeNode = {
  title: React.ReactNode;
  key: string;
  children?: TreeNode[];
  isLeaf?: boolean;
};

export default function FileBrowser({ versionId }: { versionId: string }) {
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [allFiles, setAllFiles] = useState<FileNode[]>([]);

  const fetchFiles = async () => {
    setLoading(true);
    try {
      const res = await versionApi.files(versionId);
      const files = (res as { files: FileNode[] }).files || [];
      setAllFiles(files);
      setTree(buildTree(files));
    } catch {
      setTree([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { fetchFiles(); }, [versionId]);

  useEffect(() => {
    if (!search) {
      setTree(buildTree(allFiles));
    } else {
      const lower = search.toLowerCase();
      const matched = allFiles.filter((f) => f.path.toLowerCase().includes(lower));
      setTree(buildTree(matched));
    }
  }, [search, allFiles]);

  return (
    <div>
      <Title level={2}>文件浏览器</Title>
      <Card>
        <Space style={{ marginBottom: 16 }}>
          <Input
            placeholder="搜索文件路径"
            prefix={<SearchOutlined />}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            allowClear
            style={{ width: 300 }}
          />
          <Button icon={<ReloadOutlined />} onClick={fetchFiles} loading={loading}>刷新</Button>
        </Space>

        <Spin spinning={loading}>
          {tree.length > 0 ? (
            <Tree
              treeData={tree}
              defaultExpandAll={!!search}
              showLine
              checkable
            />
          ) : (
            <Empty description="暂无文件" />
          )}
        </Spin>
      </Card>
    </div>
  );
}

function buildTree(files: FileNode[]): TreeNode[] {
  const root: TreeNode & { children: TreeNode[] } = { title: '/', key: '/', children: [] };
  for (const file of files) {
    const parts = file.path.split('/').filter(Boolean);
    let current = root;
    for (let i = 0; i < parts.length; i++) {
      const key = '/' + parts.slice(0, i + 1).join('/');
      const isLast = i === parts.length - 1;
      let child = current.children.find((c) => c.key === key);
      if (!child) {
        child = {
          title: isLast ? (
            <Space>
              <span>{parts[i]}</span>
              {file.size !== undefined && <Tag>{formatBytes(file.size)}</Tag>}
            </Space>
          ) : (
            parts[i]
          ),
          key,
          children: [],
          isLeaf: isLast && !file.is_dir,
        };
        current.children.push(child);
      }
      current = child as typeof root;
    }
  }
  return root.children;
}

function formatBytes(bytes: number): string {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}