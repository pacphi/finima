import { create } from 'zustand';

export type LlmStatus = 'loading' | 'ready' | 'failed' | 'unknown';
export type FeedStatus = 'loading' | 'ready' | 'unknown';

interface HealthState {
  llmStatus: LlmStatus;
  feedStatus: FeedStatus;
  /** Start polling /health every `intervalMs` (default 3 000 ms). Returns a
   *  cleanup function that stops the poll. */
  startPolling: (baseUrl: string, intervalMs?: number) => () => void;
}

export const useHealthStore = create<HealthState>()((set, get) => ({
  llmStatus: 'unknown',
  feedStatus: 'unknown',

  startPolling: (baseUrl: string, intervalMs = 3_000) => {
    let stopped = false;

    const poll = async () => {
      try {
        const res = await fetch(`${baseUrl}/health`);
        if (!res.ok) {
          set({ llmStatus: 'unknown', feedStatus: 'unknown' });
          return;
        }
        const body = (await res.json()) as { llm?: string; feed?: string };
        const llm = body.llm;
        const feed = body.feed;
        set({
          llmStatus: llm === 'ready' || llm === 'loading' || llm === 'failed' ? llm : 'unknown',
          feedStatus: feed === 'ready' || feed === 'loading' ? feed : 'unknown',
        });
      } catch {
        set({ llmStatus: 'unknown', feedStatus: 'unknown' });
      }
    };

    // Immediate first check.
    void poll();

    const id = setInterval(() => {
      if (stopped) return;
      // Stop polling once both statuses are terminal.
      const { llmStatus, feedStatus } = get();
      const llmDone = llmStatus === 'ready' || llmStatus === 'failed';
      const feedDone = feedStatus === 'ready';
      if (llmDone && feedDone) {
        clearInterval(id);
        return;
      }
      void poll();
    }, intervalMs);

    return () => {
      stopped = true;
      clearInterval(id);
    };
  },
}));
