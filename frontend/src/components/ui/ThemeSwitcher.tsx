import { useThemeStore, type ThemeMode } from '@/stores/themeStore';

const options: { value: ThemeMode; label: string }[] = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'System' },
];

export function ThemeSwitcher() {
  const mode = useThemeStore((s) => s.mode);
  const setMode = useThemeStore((s) => s.setMode);

  return (
    <div
      className="flex items-center gap-1 rounded-lg bg-[var(--color-surface)] border border-[var(--color-border)] p-1"
      role="radiogroup"
      aria-label="Theme mode"
    >
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => setMode(opt.value)}
          role="radio"
          aria-checked={mode === opt.value}
          className={`px-4 py-2 text-sm rounded-md transition-colors ${
            mode === opt.value
              ? 'bg-[var(--color-primary)] text-white font-medium'
              : 'text-[var(--color-text-secondary)] hover:text-[var(--color-text)] hover:bg-[var(--color-border)]'
          }`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}
