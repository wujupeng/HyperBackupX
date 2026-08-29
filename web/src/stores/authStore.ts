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
  token: null,
  username: null,
  displayName: null,
  roles: [],
  isAuthenticated: false,
  loading: false,
  error: null,

  login: async (req) => {
    set({ loading: true, error: null });
    try {
      const res = await authApi.login(req);
      set({
        token: 'cookie',
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