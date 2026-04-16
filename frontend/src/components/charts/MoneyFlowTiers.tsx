import { useState, useCallback } from 'react';
import type { SankeyData, SubcategorySpend } from '@/types/models';
import { SpendingDonut } from './SpendingDonut';
import { toTitleCase } from '@/utils/format';

interface MoneyFlowTiersProps {
  data: SankeyData;
  onLoadSubcategories: (category: string) => Promise<SubcategorySpend[]>;
}

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

const TIER_COLORS = [
  '#3B82F6',
  '#22C55E',
  '#F59E0B',
  '#EF4444',
  '#8B5CF6',
  '#EC4899',
  '#14B8A6',
  '#F97316',
  '#6366F1',
  '#84CC16',
];

interface CategoryBarData {
  name: string;
  amount: number;
  pct: number;
  color: string;
}

function CategoryBar({
  item,
  onClick,
  isSelected,
}: {
  item: CategoryBarData;
  onClick: () => void;
  isSelected: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`group flex items-center gap-2 rounded-md px-2 py-1.5 text-left transition-all hover:bg-[var(--color-bg-secondary)] ${
        isSelected ? 'ring-2 ring-[var(--color-accent)] bg-[var(--color-bg-secondary)]' : ''
      }`}
      title={`${item.name}: ${formatCurrency(item.amount)} (${item.pct.toFixed(1)}%)`}
    >
      <div
        className="h-3 rounded-sm transition-all"
        style={{
          width: `${Math.max(8, item.pct * 2)}px`,
          backgroundColor: item.color,
        }}
      />
      <span className="text-xs text-[var(--color-text)]">{item.name}</span>
      <span className="text-xs text-[var(--color-text-secondary)]">
        {formatCurrency(item.amount)}
      </span>
      <span className="text-[10px] text-[var(--color-text-secondary)]">
        ({item.pct.toFixed(0)}%)
      </span>
    </button>
  );
}

export function MoneyFlowTiers({ data, onLoadSubcategories }: MoneyFlowTiersProps) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [subcategoryData, setSubcategoryData] = useState<SubcategorySpend[] | null>(null);

  // Parse the sankey data into tiers
  const primaryNodes = data.nodes.filter((n) => n.column === 'primary');
  const secondaryNodes = data.nodes.filter((n) => n.column === 'secondary');
  const spendingNodes = data.nodes.filter((n) => n.column === 'right');

  const primaryNames = new Set(primaryNodes.map((n) => n.id));
  const secondaryNames = new Set(secondaryNodes.map((n) => n.id));
  const spendingNames = new Set(spendingNodes.map((n) => n.id));

  // Tier 1: Income → Primary account
  const incomeLinks = data.links.filter(
    (l) => !primaryNames.has(l.source) && primaryNames.has(l.target),
  );
  const totalIncome = incomeLinks.reduce((s, l) => s + l.value, 0);

  // Tier 2: Primary → Secondary (transfers)
  const transferLinks = data.links.filter(
    (l) => primaryNames.has(l.source) && secondaryNames.has(l.target),
  );

  // Tier 3: Spending by category per account
  const spendingLinks = data.links.filter((l) => spendingNames.has(l.target));

  // Group spending by source account
  const spendingByAccount = new Map<string, { name: string; amount: number }[]>();
  for (const link of spendingLinks) {
    if (!spendingByAccount.has(link.source)) spendingByAccount.set(link.source, []);
    spendingByAccount.get(link.source)!.push({ name: link.target, amount: link.value });
  }
  // Sort each account's categories by amount
  for (const [, cats] of spendingByAccount) {
    cats.sort((a, b) => b.amount - a.amount);
  }

  // Sort accounts: primary first, then secondary by total spending
  const accountOrder = [...spendingByAccount.entries()].sort((a, b) => {
    if (primaryNames.has(a[0]) && !primaryNames.has(b[0])) return -1;
    if (!primaryNames.has(a[0]) && primaryNames.has(b[0])) return 1;
    const totalA = a[1].reduce((s, c) => s + c.amount, 0);
    const totalB = b[1].reduce((s, c) => s + c.amount, 0);
    return totalB - totalA;
  });

  const handleCategoryClick = useCallback(
    async (category: string) => {
      if (category === 'Other' || category === 'Other Income' || category === 'Uncategorized') {
        setSelectedCategory(null);
        setSubcategoryData(null);
        return;
      }
      if (selectedCategory === category) {
        setSelectedCategory(null);
        setSubcategoryData(null);
        return;
      }
      setSelectedCategory(category);
      try {
        const data = await onLoadSubcategories(category);
        setSubcategoryData(data);
      } catch {
        setSubcategoryData(null);
      }
    },
    [selectedCategory, onLoadSubcategories],
  );

  if (!data.nodes.length || !data.links.length) {
    return (
      <div className="flex h-64 items-center justify-center text-[var(--color-text-secondary)]">
        No flow data available for this period
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* ── Tier 1: Income Into Primary Account ── */}
      {incomeLinks.length > 0 && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <div className="mb-3 flex items-center justify-between">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
              Income Into {primaryNodes[0]?.name ?? 'Primary Account'}
            </h4>
            <span className="text-sm font-medium text-green-500">
              +{formatCurrency(totalIncome)}
            </span>
          </div>

          {/* Stacked bar */}
          <div className="mb-3 flex h-8 overflow-hidden rounded-md">
            {incomeLinks
              .sort((a, b) => b.value - a.value)
              .map((link, i) => (
                <div
                  key={link.source}
                  className="relative flex items-center justify-center transition-all hover:opacity-80"
                  style={{
                    width: `${Math.max(2, (link.value / totalIncome) * 100)}%`,
                    backgroundColor: TIER_COLORS[i % TIER_COLORS.length],
                  }}
                  title={`${link.source}: ${formatCurrency(link.value)}`}
                >
                  {link.value / totalIncome > 0.15 && (
                    <span className="truncate px-1 text-[10px] font-medium text-white">
                      {link.source}
                    </span>
                  )}
                </div>
              ))}
          </div>

          {/* Legend */}
          <div className="flex flex-wrap gap-3">
            {incomeLinks
              .sort((a, b) => b.value - a.value)
              .map((link, i) => (
                <div key={link.source} className="flex items-center gap-1.5">
                  <div
                    className="h-2.5 w-2.5 rounded-sm"
                    style={{ backgroundColor: TIER_COLORS[i % TIER_COLORS.length] }}
                  />
                  <span className="text-xs text-[var(--color-text)]">{link.source}</span>
                  <span className="text-xs text-[var(--color-text-secondary)]">
                    {formatCurrency(link.value)}
                  </span>
                </div>
              ))}
          </div>
        </div>
      )}

      {/* ── Tier 2: Transfers to Credit Cards ── */}
      {transferLinks.length > 0 && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <h4 className="mb-3 text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
            Transfers from {primaryNodes[0]?.name ?? 'Primary'}
          </h4>
          <div className="space-y-2">
            {transferLinks
              .sort((a, b) => b.value - a.value)
              .map((link, i) => {
                const pct = totalIncome > 0 ? (link.value / totalIncome) * 100 : 0;
                return (
                  <div key={link.target} className="flex items-center gap-3">
                    <span className="w-44 truncate text-sm text-[var(--color-text)]">
                      {link.target}
                    </span>
                    <div className="flex-1">
                      <div className="h-5 overflow-hidden rounded-md bg-[var(--color-bg-secondary)]">
                        <div
                          className="flex h-full items-center rounded-md px-2 transition-all"
                          style={{
                            width: `${Math.max(2, pct)}%`,
                            backgroundColor: TIER_COLORS[(i + 4) % TIER_COLORS.length],
                          }}
                        >
                          {pct > 10 && (
                            <span className="text-[10px] font-medium text-white">
                              {formatCurrency(link.value)}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                    <span className="w-20 text-right text-sm font-medium text-[var(--color-text)]">
                      {formatCurrency(link.value)}
                    </span>
                    <span className="w-12 text-right text-xs text-[var(--color-text-secondary)]">
                      {pct.toFixed(0)}%
                    </span>
                  </div>
                );
              })}
          </div>
        </div>
      )}

      {/* ── Tier 3: Spending by Category (per account) ── */}
      {accountOrder.length > 0 && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <h4 className="mb-3 text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
            Spending by Category
          </h4>
          <div className="space-y-4">
            {accountOrder.map(([accountName, categories]) => {
              const accountTotal = categories.reduce((s, c) => s + c.amount, 0);
              const bars: CategoryBarData[] = categories.map((c, i) => ({
                name: c.name,
                amount: c.amount,
                pct: accountTotal > 0 ? (c.amount / accountTotal) * 100 : 0,
                color: TIER_COLORS[i % TIER_COLORS.length]!,
              }));

              return (
                <div key={accountName}>
                  <div className="mb-2 flex items-center justify-between">
                    <span className="text-sm font-medium text-[var(--color-text)]">
                      {accountName}
                    </span>
                    <span className="text-xs text-[var(--color-text-secondary)]">
                      {formatCurrency(accountTotal)} total
                    </span>
                  </div>

                  {/* Stacked horizontal bar */}
                  <div className="mb-2 flex h-6 overflow-hidden rounded-md">
                    {bars.map((bar) => (
                      <div
                        key={bar.name}
                        className="relative cursor-pointer transition-all hover:opacity-80"
                        style={{
                          width: `${Math.max(1, bar.pct)}%`,
                          backgroundColor: bar.color,
                        }}
                        onClick={() => void handleCategoryClick(bar.name)}
                        title={`${bar.name}: ${formatCurrency(bar.amount)} (${bar.pct.toFixed(0)}%)`}
                      >
                        {bar.pct > 12 && (
                          <span className="absolute inset-0 flex items-center justify-center truncate px-1 text-[10px] font-medium text-white">
                            {bar.name}
                          </span>
                        )}
                      </div>
                    ))}
                  </div>

                  {/* Category labels */}
                  <div className="flex flex-wrap gap-1">
                    {bars.map((bar) => (
                      <CategoryBar
                        key={bar.name}
                        item={bar}
                        onClick={() => void handleCategoryClick(bar.name)}
                        isSelected={selectedCategory === bar.name}
                      />
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* ── Subcategory Donut Drill-down ── */}
      {selectedCategory && subcategoryData && subcategoryData.length > 0 && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
              {toTitleCase(selectedCategory)} Breakdown
            </h4>
            <button
              onClick={() => {
                setSelectedCategory(null);
                setSubcategoryData(null);
              }}
              className="text-xs text-[var(--color-text-secondary)] hover:text-[var(--color-text)]"
            >
              Close
            </button>
          </div>
          <SpendingDonut
            data={[]}
            selectedCategory={selectedCategory}
            subcategoryData={subcategoryData}
            onDismissSubcategory={() => {
              setSelectedCategory(null);
              setSubcategoryData(null);
            }}
          />
        </div>
      )}
    </div>
  );
}
