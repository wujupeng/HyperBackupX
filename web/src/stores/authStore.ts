import { create } from 'zustand';
import { authApi } from '../api/endpoints';
import type { LoginRequest } from '../api/types';

interface AuthState {
  token: string | null;
  username: string | null;
  displayName: string | null;
  roles: string[];
  isAuthenticated: boolean;
  loading: boolean;
  error: string | null;
  login: (req: LoginRequest) => Promise<boolean>;
  logout: () => void;
  clearError: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  token: localStorage.getItem('hbx_token'),
  username: localStorage.getItem('hbx_username'),
  displayName: localStorage.getItem('hbx_display_name'),
  roles: JSON.parse(localStorage.getItem('hbx_roles') || '[]'),
  isAuthenticated: !!localStorage.getItem('hbx_token'),
  loading: false,
  error: null,

  login: async (req) => {
    set({ loading: true, error: null });
    try {
      const res = await authApi.login(req);
      localStorage.setItem('hbx_token', res.token);
      localStorage.setItem('hbx_username', res.username);
      localStorage.setItem('hbx_display_name', res.display_name);
      localStorage.setItem('hbx_roles', JSON.stringify(res.roles));
      set({
        token: res.token,
        username: res.username,
        displayName: res.display_name,
        roles: res.roles,
        isAuthenticated: true,
        loading: false,
      });
      return true;
    } catch (err: unknown) {
      const error = err as { response?: { data?: { error?: string } } };
      set({
        loading: false,
        error: error.response?.data?.error || '登录失败',
      });
      return false;
    }
  },

  logout: () => {
    authApi.logout().catch(() => {});
    localStorage.removeItem('hbx_token');
    localStorage.removeItem('hbx_username');
    localStorage.removeItem('hbx_display_name');
    localStorage.removeItem('hbx_roles');
    set({
      token: null,
      username: null,
      displayName: null,
      roles: [],
      isAuthenticated: false,
    });
  },

  clearError: () => set({ error: null }),
}));