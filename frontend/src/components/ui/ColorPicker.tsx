import { useState } from 'react';
import { useThemeStore } from '@/stores/themeStore';

const PRESET_COLORS = [
  { hex: '#3B82F6', name: 'Blue' },
  { hex: '#8B5CF6', name: 'Purple' },
  { hex: '#EC4899', name: 'Pink' },
  { hex: '#EF4444', name: 'Red' },
  { hex: '#F97316', name: 'Orange' },
  { hex: '#EAB308', name: 'Yellow' },
  { hex: '#22C55E', name: 'Green' },
  { hex: '#14B8A6', name: 'Teal' },
];

export function ColorPicker() {
  const accentColor = useThemeStore((s) => s.accentColor);
  const setAccentColor = useThemeStore((s) => s.setAccentColor);
  const [customHex, setCustomHex] = useState(accentColor);

  const handleCustomChange = (value: string) => {
    setCustomHex(value);
    if (/^#[0-9A-Fa-f]{6}$/.test(value)) {
      setAccentColor(value);
    }
  };

  return (
    <div className="space-y-3">
      <label id="color-picker-label" className="block text-sm font-medium text-[var(--color-text)]">
        Accent Color
      </label>

      {/* Preset palette */}
      <div className="flex flex-wrap gap-2" role="radiogroup" aria-labelledby="color-picker-label">
        {PRESET_COLORS.map((color) => {
          const isSelected = accentColor.toLowerCase() === color.hex.toLowerCase();
          return (
            <button
              key={color.hex}
              onClick={() => {
                setAccentColor(color.hex);
                setCustomHex(color.hex);
              }}
              role="radio"
              aria-checked={isSelected}
              aria-label={`${color.name}${isSelected ? ' (selected)' : ''}`}
              className={`w-10 h-10 rounded-lg border-2 transition-all ${
                isSelected
                  ? 'border-[var(--color-text)] scale-110'
                  : 'border-transparent hover:border-[var(--color-border)]'
              }`}
              style={{ backgroundColor: color.hex }}
            />
          );
        })}
      </div>

      {/* Custom hex input */}
      <div className="flex items-center gap-2">
        <label htmlFor="custom-hex-input" className="text-sm text-[var(--color-text-secondary)]">
          Custom:
        </label>
        <input
          id="custom-hex-input"
          type="text"
          value={customHex}
          onChange={(e) => handleCustomChange(e.target.value)}
          placeholder="#3B82F6"
          maxLength={7}
          aria-describedby="custom-hex-preview"
          className="w-28 px-2 py-1.5 text-sm border border-[var(--color-input-border)] bg-[var(--color-input-bg)] text-[var(--color-text)] rounded-lg focus:border-[var(--color-primary)] focus:ring-1 focus:ring-[var(--color-primary)]"
        />
        <div
          id="custom-hex-preview"
          className="w-8 h-8 rounded-md border border-[var(--color-border)]"
          style={{ backgroundColor: accentColor }}
          role="img"
          aria-label={`Current accent color: ${accentColor}`}
        />
      </div>
    </div>
  );
}
