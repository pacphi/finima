import { create } from 'zustand';
import yaml from 'js-yaml';

interface ConfigState {
  apiBaseUrl: string;
  wsUrl: string;
  features: {
    newsFeed: boolean;
    flowAnalysis: boolean;
  };
  defaults: {
    currency: string;
    dateFormat: string;
    theme: string;
  };
  loaded: boolean;
  loadConfig: () => Promise<void>;
}

interface RawConfig {
  api?: {
    base_url?: string;
    ws_url?: string;
  };
  features?: {
    news_feed?: boolean;
    flow_analysis?: boolean;
  };
  defaults?: {
    currency?: string;
    date_format?: string;
    theme?: string;
  };
}

export const useConfigStore = create<ConfigState>()((set) => ({
  apiBaseUrl: 'http://localhost:3000',
  wsUrl: 'ws://localhost:3000',
  features: {
    newsFeed: true,
    flowAnalysis: true,
  },
  defaults: {
    currency: 'USD',
    dateFormat: 'MM/DD/YYYY',
    theme: 'system',
  },
  loaded: false,

  loadConfig: async () => {
    try {
      const response = await fetch('/config.yaml');
      const text = await response.text();
      const raw = yaml.load(text) as RawConfig;

      set({
        apiBaseUrl: raw.api?.base_url ?? 'http://localhost:3000',
        wsUrl: raw.api?.ws_url ?? 'ws://localhost:3000',
        features: {
          newsFeed: raw.features?.news_feed ?? true,
          flowAnalysis: raw.features?.flow_analysis ?? true,
        },
        defaults: {
          currency: raw.defaults?.currency ?? 'USD',
          dateFormat: raw.defaults?.date_format ?? 'MM/DD/YYYY',
          theme: raw.defaults?.theme ?? 'system',
        },
        loaded: true,
      });
    } catch (error) {
      console.error('Failed to load config.yaml:', error);
      set({ loaded: true });
    }
  },
}));
