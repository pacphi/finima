import { describe, it, expect, beforeEach } from 'vitest';
import { useAuthStore } from '../authStore';

describe('authStore', () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: null,
      accessToken: null,
      refreshToken: null,
      isAuthenticated: false,
    });
  });

  it('should start with unauthenticated state', () => {
    const state = useAuthStore.getState();
    expect(state.user).toBeNull();
    expect(state.accessToken).toBeNull();
    expect(state.refreshToken).toBeNull();
    expect(state.isAuthenticated).toBe(false);
  });

  it('should set user and tokens on login', () => {
    const user = { id: '1', email: 'test@example.com', displayName: 'Test User' };
    const tokens = { accessToken: 'access-123', refreshToken: 'refresh-456' };

    useAuthStore.getState().login(user, tokens);

    const state = useAuthStore.getState();
    expect(state.user).toEqual(user);
    expect(state.accessToken).toBe('access-123');
    expect(state.refreshToken).toBe('refresh-456');
    expect(state.isAuthenticated).toBe(true);
  });

  it('should clear state on logout', () => {
    // First login
    useAuthStore
      .getState()
      .login(
        { id: '1', email: 'test@example.com', displayName: 'Test' },
        { accessToken: 'a', refreshToken: 'r' },
      );

    // Then logout
    useAuthStore.getState().logout();

    const state = useAuthStore.getState();
    expect(state.user).toBeNull();
    expect(state.accessToken).toBeNull();
    expect(state.refreshToken).toBeNull();
    expect(state.isAuthenticated).toBe(false);
  });

  it('should update tokens with setTokens', () => {
    useAuthStore
      .getState()
      .login(
        { id: '1', email: 'test@example.com', displayName: 'Test' },
        { accessToken: 'old-access', refreshToken: 'old-refresh' },
      );

    useAuthStore.getState().setTokens('new-access', 'new-refresh');

    const state = useAuthStore.getState();
    expect(state.accessToken).toBe('new-access');
    expect(state.refreshToken).toBe('new-refresh');
    // User should remain unchanged
    expect(state.user?.email).toBe('test@example.com');
  });

  it('should derive isAuthenticated correctly', () => {
    expect(useAuthStore.getState().isAuthenticated).toBe(false);

    useAuthStore
      .getState()
      .login(
        { id: '1', email: 'test@example.com', displayName: 'Test' },
        { accessToken: 'a', refreshToken: 'r' },
      );
    expect(useAuthStore.getState().isAuthenticated).toBe(true);

    useAuthStore.getState().logout();
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });
});
