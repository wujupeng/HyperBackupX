import { create } from 'zustand';
import { monitoringApi } from '../api/endpoints';
import type { DashboardData } from '../api/types';

interface DashboardState {
  data: DashboardData | null;
  loading: boolean;
  error: string | null;
  fetchDashboard: () => Promise<void>;
}

export const useDashboardStore = create<DashboardState>((set) => ({
  data: null,
  loading: false,
  error: null,

  fetchDashboard: async () => {
    set({ loading: true, error: null });
    try {
      const data = await monitoringApi.dashboard();
      set({ data, loading: false });
    } catch (err: unknown) {
      const error = err as { response?: { data?: { error?: string } } };
      set({
        loading: false,
        error: error.response?.data?.error || '获取仪表盘数据失败',
      });
    }
  },
}));