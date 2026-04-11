import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useConfigStore } from '../configStore';

describe('configStore', () => {
  beforeEach(() => {
    useConfigStore.setState({
      apiBaseUrl: 'http://localhost:3000',
      wsUrl: 'ws://localhost:3000',
      features: { newsFeed: true, flowAnalysis: true },
      defaults: { currency: 'USD', dateFormat: 'MM/DD/YYYY', theme: 'system' },
      loaded: false,
    });
    vi.restoreAllMocks();
  });

  it('should have correct initial defaults', () => {
    const state = useConfigStore.getState();
    expect(state.apiBaseUrl).toBe('http://localhost:3000');
    expect(state.wsUrl).toBe('ws://localhost:3000');
    expect(state.features.newsFeed).toBe(true);
    expect(state.features.flowAnalysis).toBe(true);
    expect(state.defaults.currency).toBe('USD');
    expect(state.defaults.dateFormat).toBe('MM/DD/YYYY');
    expect(state.defaults.theme).toBe('system');
    expect(state.loaded).toBe(false);
  });

  it('should parse YAML config correctly via loadConfig', async () => {
    const yamlContent = `
api:
  base_url: "https://api.finima.app"
  ws_url: "wss://api.finima.app"
features:
  news_feed: false
  flow_analysis: true
defaults:
  currency: "EUR"
  date_format: "DD/MM/YYYY"
  theme: "dark"
`;

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      text: () => Promise.resolve(yamlContent),
    } as Response);

    await useConfigStore.getState().loadConfig();

    const state = useConfigStore.getState();
    expect(state.apiBaseUrl).toBe('https://api.finima.app');
    expect(state.wsUrl).toBe('wss://api.finima.app');
    expect(state.features.newsFeed).toBe(false);
    expect(state.features.flowAnalysis).toBe(true);
    expect(state.defaults.currency).toBe('EUR');
    expect(state.defaults.dateFormat).toBe('DD/MM/YYYY');
    expect(state.defaults.theme).toBe('dark');
    expect(state.loaded).toBe(true);
  });

  it('should handle fetch failure gracefully', async () => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new Error('Network error'));

    await useConfigStore.getState().loadConfig();

    const state = useConfigStore.getState();
    // Should fall back to defaults
    expect(state.apiBaseUrl).toBe('http://localhost:3000');
    expect(state.loaded).toBe(true);
  });

  it('should handle partial YAML config', async () => {
    const yamlContent = `
api:
  base_url: "https://custom.api"
`;

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      text: () => Promise.resolve(yamlContent),
    } as Response);

    await useConfigStore.getState().loadConfig();

    const state = useConfigStore.getState();
    expect(state.apiBaseUrl).toBe('https://custom.api');
    // Defaults for missing fields
    expect(state.wsUrl).toBe('ws://localhost:3000');
    expect(state.features.newsFeed).toBe(true);
    expect(state.defaults.currency).toBe('USD');
    expect(state.loaded).toBe(true);
  });
});
