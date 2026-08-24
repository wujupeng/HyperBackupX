import ReactECharts from 'echarts-for-react';
import type { DashboardData } from '../api/types';

export function DeviceStatusChart({ data }: { data: DashboardData | null }) {
  const online = data?.devices.online ?? 0;
  const offline = (data?.devices.total ?? 0) - online;
  const option = {
    tooltip: { trigger: 'item' },
    legend: { bottom: 0 },
    series: [
      {
        name: '设备状态',
        type: 'pie',
        radius: ['40%', '70%'],
        avoidLabelOverlap: false,
        label: { show: true, formatter: '{b}: {c}' },
        data: [
          { value: online, name: '在线', itemStyle: { color: '#52c41a' } },
          { value: offline, name: '离线', itemStyle: { color: '#d9d9d9&' } },
        ],
      },
    ],
  };
  return <ReactECharts option={option} style={{ height: 280 }} />;
}

export function JobStatusChart({ data }: { data: DashboardData | null }) {
  const active = data?.jobs.active ?? 0;
  const inactive = (data?.jobs.total ?? 0) - active;
  const option = {
    tooltip: { trigger: 'item' },
    legend: { bottom: 0 },
    series: [
      {
        name: '任务状态',
        type: 'pie',
        radius: '65%',
        data: [
          { value: active, name: '活跃', itemStyle: { color: '#1677ff' } },
          { value: inactive, name: '非活跃', itemStyle: { color: '#bfbfbf' } },
        ],
      },
    ],
  };
  return <ReactECharts option={option} style={{ height: 280 }} />;
}

export function StorageChart({ data }: { data: DashboardData | null }) {
  const totalSize = data?.versions.total_size ?? 0;
  const option = {
    tooltip: { formatter: '{b}: {c} bytes' },
    series: [
      {
        type: 'gauge',
        min: 0,
        max: Math.max(totalSize, 1),
        progress: { show: true, width: 18 },
        axisLine: { lineStyle: { width: 18 } },
        detail: {
          valueAnimation: true,
          formatter: (val: number) => formatBytes(val),
        },
        data: [{ value: totalSize, name: '总备份量' }],
      },
    ],
  };
  return <ReactECharts option={option} style={{ height: 280 }} />;
}

export function ThroughputChart() {
  const hours = Array.from({ length: 24 }, (_, i) => `${i}:00`);
  const throughput = hours.map(() => Math.round(50 + Math.random() * 150));
  const option = {
    tooltip: { trigger: 'axis' },
    xAxis: { type: 'category', data: hours },
    yAxis: { type: 'value', name: 'MB/s' },
    series: [
      {
        name: '吞吐量',
        type: 'line',
        smooth: true,
        areaStyle: { opacity: 0.3 },
        data: throughput,
        itemStyle: { color: '#1677ff' },
      },
    ],
  };
  return <ReactECharts option={option} style={{ height: 280 }} />;
}

export function DurationChart() {
  const option = {
    tooltip: { trigger: 'axis' },
    legend: { bottom: 0 },
    xAxis: { type: 'category', data: ['<1m', '1-5m', '5-15m', '15-30m', '30-60m', '>60m'] },
    yAxis: { type: 'value', name: '任务数' },
    series: [
      {
        name: '全量备份',
        type: 'bar',
        data: [12, 18, 8, 5, 3, 1],
        itemStyle: { color: '#1677ff' },
      },
      {
        name: '增量备份',
        type: 'bar',
        data: [45, 30, 12, 6, 2, 0],
        itemStyle: { color: '#52c41a' },
      },
    ],
  };
  return <ReactECharts option={option} style={{ height: 280 }} />;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}