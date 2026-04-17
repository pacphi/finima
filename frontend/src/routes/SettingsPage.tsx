import { useState, useEffect } from 'react';
import { ThemeSwitcher } from '@/components/ui/ThemeSwitcher';
import { ColorPicker } from '@/components/ui/ColorPicker';
import { useThemeStore } from '@/stores/themeStore';
import { usePrefsStore } from '@/stores/prefsStore';
import { useApi } from '@/hooks/useApi';
import { useConfigStore } from '@/stores/configStore';
import { CategoriesTab } from '@/components/settings/CategoriesTab';

type TabId = 'theme' | 'layout' | 'general' | 'llm' | 'categories';

const tabs: { id: TabId; label: string }[] = [
  { id: 'theme', label: 'Theme' },
  { id: 'layout', label: 'Layout' },
  { id: 'general', label: 'General' },
  { id: 'categories', label: 'Categories' },
  { id: 'llm', label: 'LLM' },
];

export function SettingsPage() {
  const [activeTab, setActiveTab] = useState<TabId>('theme');
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);
  const api = useApi();

  const saveToApi = usePrefsStore((s) => s.saveToApi);

  const handleSave = async () => {
    setSaving(true);
    setSaveMsg(null);
    try {
      await saveToApi(api.put);
      setSaveMsg('Preferences saved.');
    } catch {
      setSaveMsg('Failed to save preferences.');
    } finally {
      setSaving(false);
      setTimeout(() => setSaveMsg(null), 3000);
    }
  };

  return (
    <div className="p-6 max-w-4xl">
      <h1 className="text-2xl font-bold text-[var(--color-text)] mb-6">Settings</h1>

      {/* Tab navigation */}
      <div
        className="flex gap-1 border-b border-[var(--color-border)] mb-6"
        role="tablist"
        aria-label="Settings sections"
      >
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            role="tab"
            aria-selected={activeTab === tab.id}
            aria-controls={`settings-tabpanel-${tab.id}`}
            id={`settings-tab-${tab.id}`}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab.id
                ? 'border-[var(--color-primary)] text-[var(--color-primary)]'
                : 'border-transparent text-[var(--color-text-secondary)] hover:text-[var(--color-text)]'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="space-y-6">
        {activeTab === 'theme' && (
          <div role="tabpanel" id="settings-tabpanel-theme" aria-labelledby="settings-tab-theme">
            <ThemeTab />
          </div>
        )}
        {activeTab === 'layout' && (
          <div role="tabpanel" id="settings-tabpanel-layout" aria-labelledby="settings-tab-layout">
            <LayoutTab />
          </div>
        )}
        {activeTab === 'general' && (
          <div
            role="tabpanel"
            id="settings-tabpanel-general"
            aria-labelledby="settings-tab-general"
          >
            <GeneralTab />
          </div>
        )}
        {activeTab === 'categories' && (
          <div
            role="tabpanel"
            id="settings-tabpanel-categories"
            aria-labelledby="settings-tab-categories"
          >
            <CategoriesTab />
          </div>
        )}
        {activeTab === 'llm' && (
          <div role="tabpanel" id="settings-tabpanel-llm" aria-labelledby="settings-tab-llm">
            <LlmTab />
          </div>
        )}
      </div>

      {/* Save button */}
      <div className="mt-8 flex items-center gap-4">
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-6 py-2.5 bg-[var(--color-primary)] text-white rounded-lg hover:bg-[var(--color-primary-hover)] transition-colors disabled:opacity-50 font-medium"
        >
          {saving ? 'Saving...' : 'Save Preferences'}
        </button>
        {saveMsg && (
          <span
            className={`text-sm ${
              saveMsg.includes('Failed')
                ? 'text-[var(--color-error)]'
                : 'text-[var(--color-primary)]'
            }`}
            role="status"
            aria-live="polite"
          >
            {saveMsg}
          </span>
        )}
      </div>
    </div>
  );
}

// ── Theme Tab ───────────────────────────────────────────────────────

function ThemeTab() {
  const mode = useThemeStore((s) => s.mode);
  const accentColor = useThemeStore((s) => s.accentColor);

  return (
    <div className="space-y-8">
      <div>
        <h3 className="text-lg font-medium text-[var(--color-text)] mb-3">Appearance</h3>
        <ThemeSwitcher />
      </div>

      <div>
        <ColorPicker />
      </div>

      {/* Live preview card */}
      <div>
        <h3 className="text-lg font-medium text-[var(--color-text)] mb-3">Preview</h3>
        <div className="p-6 rounded-xl border border-[var(--color-border)] bg-[var(--color-card)] max-w-sm">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-10 h-10 rounded-full" style={{ backgroundColor: accentColor }} />
            <div>
              <p className="font-medium text-[var(--color-text)]">Preview Card</p>
              <p className="text-xs text-[var(--color-text-secondary)]">
                Mode: {mode} | Accent: {accentColor}
              </p>
            </div>
          </div>
          <div className="space-y-2">
            <div className="h-2 rounded-full bg-[var(--color-border)] w-full" />
            <div className="h-2 rounded-full w-3/4" style={{ backgroundColor: accentColor }} />
            <div className="h-2 rounded-full bg-[var(--color-border)] w-1/2" />
          </div>
          <button
            className="mt-4 px-4 py-2 text-sm text-white rounded-lg transition-colors"
            style={{ backgroundColor: accentColor }}
          >
            Sample Button
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Layout Tab ──────────────────────────────────────────────────────

function LayoutTab() {
  const widgets = usePrefsStore((s) => s.dashboardWidgets);
  const toggleWidget = usePrefsStore((s) => s.toggleWidget);
  const resetDashboardLayout = usePrefsStore((s) => s.resetDashboardLayout);

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-[var(--color-text)] mb-3">Dashboard Widgets</h3>
        <p className="text-sm text-[var(--color-text-secondary)] mb-4">
          Choose which widgets to display on your dashboard.
        </p>
        <div className="space-y-2">
          {widgets.map((widget) => (
            <label
              key={widget.id}
              className="flex items-center gap-3 p-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] cursor-pointer hover:bg-[var(--color-surface)] transition-colors"
            >
              <input
                type="checkbox"
                checked={widget.visible}
                onChange={() => toggleWidget(widget.id)}
                className="w-4 h-4 rounded border-[var(--color-input-border)] text-[var(--color-primary)]"
              />
              <span className="text-sm text-[var(--color-text)]">{widget.label}</span>
            </label>
          ))}
        </div>
      </div>

      <button
        onClick={resetDashboardLayout}
        className="px-4 py-2 text-sm border border-[var(--color-border)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-surface)] transition-colors"
      >
        Reset to Default Layout
      </button>
    </div>
  );
}

// ── General Tab ─────────────────────────────────────────────────────

function GeneralTab() {
  const currency = usePrefsStore((s) => s.currency);
  const dateFormat = usePrefsStore((s) => s.dateFormat);
  const fiscalMonth = usePrefsStore((s) => s.fiscalMonth);
  const defaultChartType = usePrefsStore((s) => s.defaultChartType);
  const updatePref = usePrefsStore((s) => s.updatePref);

  const currencies = ['USD', 'EUR', 'GBP', 'CAD', 'AUD', 'JPY', 'CHF', 'NZD'];
  const dateFormats = ['MM/DD/YYYY', 'DD/MM/YYYY', 'YYYY-MM-DD'];
  const months = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ];
  const chartTypes = [
    { value: 'line', label: 'Line' },
    { value: 'bar', label: 'Bar' },
    { value: 'area', label: 'Area' },
  ];

  return (
    <div className="space-y-6 max-w-md">
      <div>
        <label className="block text-sm font-medium text-[var(--color-text)] mb-1">Currency</label>
        <select
          value={currency}
          onChange={(e) => updatePref('currency', e.target.value)}
          className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        >
          {currencies.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
          Date Format
        </label>
        <select
          value={dateFormat}
          onChange={(e) => updatePref('dateFormat', e.target.value)}
          className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        >
          {dateFormats.map((f) => (
            <option key={f} value={f}>
              {f}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
          Fiscal Year Start Month
        </label>
        <select
          value={fiscalMonth}
          onChange={(e) => updatePref('fiscalMonth', Number(e.target.value))}
          className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        >
          {months.map((m, i) => (
            <option key={m} value={i + 1}>
              {m}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
          Default Chart Type
        </label>
        <select
          value={defaultChartType}
          onChange={(e) => updatePref('defaultChartType', e.target.value)}
          className="w-full px-3 py-2 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        >
          {chartTypes.map((ct) => (
            <option key={ct.value} value={ct.value}>
              {ct.label}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}

function LlmTab() {
  const config = useConfigStore.getState();
  const [llmStatus, setLlmStatus] = useState<
    'checking' | 'ready' | 'loading' | 'failed' | 'disabled' | 'unreachable'
  >('checking');
  const [provider, setProvider] = useState<string>('');
  const [model, setModel] = useState<string>('');
  const [endpoint, setEndpoint] = useState<string>('');

  // Fetch LLM info from the health endpoint on mount.
  useEffect(() => {
    const checkConnection = async () => {
      try {
        const response = await fetch(`${config.apiBaseUrl}/health`);
        if (response.ok) {
          const data = await response.json();
          setProvider(data.llm_provider ?? '');
          setModel(data.llm_model ?? '');
          setEndpoint(data.llm_endpoint ?? '');
          const status = data.llm as string;
          if (status === 'ready') setLlmStatus('ready');
          else if (status === 'loading') setLlmStatus('loading');
          else if (status === 'disabled') setLlmStatus('disabled');
          else setLlmStatus('failed');
        } else {
          setLlmStatus('unreachable');
        }
      } catch {
        setLlmStatus('unreachable');
      }
    };
    void checkConnection();
  }, [config.apiBaseUrl]);

  const providerLabel =
    provider === 'ollama'
      ? 'Ollama'
      : provider === 'candle'
        ? 'Candle (in-process)'
        : provider === 'none'
          ? 'None (disabled)'
          : provider || '—';

  const statusColor =
    llmStatus === 'ready'
      ? 'bg-[var(--color-success)]'
      : llmStatus === 'loading'
        ? 'bg-yellow-400 animate-pulse'
        : llmStatus === 'disabled'
          ? 'bg-gray-400'
          : 'bg-[var(--color-error)]';

  const statusLabel =
    llmStatus === 'ready'
      ? 'Connected'
      : llmStatus === 'loading'
        ? 'Loading model…'
        : llmStatus === 'disabled'
          ? 'Disabled'
          : llmStatus === 'failed'
            ? 'Failed to load'
            : llmStatus === 'unreachable'
              ? 'Server unreachable'
              : 'Checking…';

  return (
    <div className="space-y-6 max-w-md">
      <div>
        <h3 className="text-lg font-medium text-[var(--color-text)] mb-3">LLM Configuration</h3>
        <p className="text-sm text-[var(--color-text-secondary)] mb-4">
          These values are configured on the server. They are displayed here for reference.
        </p>
      </div>

      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
            Provider
          </label>
          <div className="px-3 py-2 text-sm border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] rounded-lg">
            {llmStatus === 'checking' ? 'Loading…' : providerLabel}
          </div>
        </div>

        {llmStatus !== 'disabled' && (
          <>
            <div>
              <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
                Model
              </label>
              <div className="px-3 py-2 text-sm border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] rounded-lg">
                {llmStatus === 'checking' ? 'Loading…' : model || '—'}
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
                Endpoint URL
              </label>
              <div className="px-3 py-2 text-sm border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] rounded-lg">
                {llmStatus === 'checking' ? 'Loading…' : endpoint || '—'}
              </div>
            </div>
          </>
        )}

        <div>
          <label className="block text-sm font-medium text-[var(--color-text)] mb-1">
            Connection Status
          </label>
          <div
            className="flex items-center gap-2 px-3 py-2 text-sm border border-[var(--color-border)] bg-[var(--color-surface)] rounded-lg"
            aria-live="polite"
          >
            <span className={`w-3 h-3 rounded-full ${statusColor}`} aria-hidden="true" />
            <span className="text-[var(--color-text)]">{statusLabel}</span>
          </div>
          {llmStatus === 'disabled' && (
            <p className="text-xs text-[var(--color-text-secondary)] mt-2">
              No LLM provider configured. Transactions are categorized using merchant lookup and
              pattern matching. To enable AI categorization, set provider to &apos;ollama&apos; or
              &apos;candle&apos; in config/llm.yaml.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
