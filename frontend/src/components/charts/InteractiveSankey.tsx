import { useState, useMemo, useCallback } from 'react';
import { PieChart, Pie, Cell, ResponsiveContainer, Legend } from 'recharts';
import type { SankeyData, SubcategorySpend } from '@/types/models';
import { SankeyDiagram } from './SankeyDiagram';
import { toTitleCase } from '@/utils/format';

/** Money-flow Sankey with a single interactive drill: clicking a
 *  spending category opens a subcategory donut beside the diagram.
 *  The Sankey itself is a strict 4-column DAG provided by the
 *  backend (see ADR-008 Amendment 2):
 *
 *    Income → Primary hub → Secondary accounts (incl. spender-role
 *                                                virtual nodes) → Categories
 *
 *  Account nodes (primary, secondary, spender-role) are non-interactive.
 *  Only category leaves respond to clicks. */
interface InteractiveSankeyProps {
  data: SankeyData;
  onLoadSubcategories: (category: string) => Promise<SubcategorySpend[]>;
}

export function InteractiveSankey({ data, onLoadSubcategories }: InteractiveSankeyProps) {
  const [donutCategory, setDonutCategory] = useState<string | null>(null);
  const [subcategoryData, setSubcategoryData] = useState<SubcategorySpend[] | null>(null);
  const [loadingSub, setLoadingSub] = useState(false);

  const categoryNames = useMemo(
    () => new Set(data.nodes.filter((n) => n.column === 'right').map((n) => n.id)),
    [data.nodes],
  );

  const dismissDonut = useCallback(() => {
    setDonutCategory(null);
    setSubcategoryData(null);
  }, []);

  const handleNodeClick = useCallback(
    async (nodeName: string) => {
      // Only category leaves are interactive. Account / spender-role
      // / income-source nodes are display-only.
      if (!categoryNames.has(nodeName)) return;
      if (nodeName === 'Other' || nodeName === 'Uncategorized') return;
      if (donutCategory === nodeName) {
        dismissDonut();
        return;
      }
      setDonutCategory(nodeName);
      setLoadingSub(true);
      try {
        setSubcategoryData(await onLoadSubcategories(nodeName));
      } catch {
        setSubcategoryData(null);
      } finally {
        setLoadingSub(false);
      }
    },
    [categoryNames, donutCategory, dismissDonut, onLoadSubcategories],
  );

  if (!data.nodes.length || !data.links.length) {
    return (
      <div className="flex h-64 items-center justify-center text-[var(--color-text-secondary)]">
        No flow data available for this period
      </div>
    );
  }

  const showDonut = donutCategory !== null;

  return (
    <div className="flex gap-4">
      {/* Sankey tile */}
      <div
        className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4 transition-all duration-300"
        style={{ width: showDonut ? '60%' : '100%' }}
      >
        <div className="overflow-x-auto">
          <SankeyDiagram
            data={data}
            width={showDonut ? 520 : 880}
            height={Math.max(380, data.nodes.length * 40)}
            onCategoryClick={(cat) => void handleNodeClick(cat)}
          />
        </div>
        <p className="mt-3 text-xs text-[var(--color-text-secondary)]">
          Click a spending category to see its subcategory breakdown.
        </p>
      </div>

      {/* Donut breakdown tile */}
      {showDonut && (
        <div
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4 transition-all duration-300"
          style={{ width: '40%', animation: 'fadeSlideIn 0.3s ease-out' }}
        >
          <div className="mb-3 flex items-center justify-between">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
              {toTitleCase(donutCategory)} Breakdown
            </h4>
            <button
              onClick={dismissDonut}
              className="rounded px-2 py-0.5 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text)] transition-colors"
            >
              Close
            </button>
          </div>
          {loadingSub ? (
            <p className="py-8 text-center text-sm text-[var(--color-text-secondary)]">Loading…</p>
          ) : subcategoryData && subcategoryData.length > 0 ? (
            <ResponsiveContainer width="100%" height={280}>
              <PieChart>
                <Pie
                  data={subcategoryData}
                  cx="50%"
                  cy="45%"
                  innerRadius={50}
                  outerRadius={85}
                  paddingAngle={2}
                  dataKey="amount"
                  nameKey="subcategory"
                >
                  {subcategoryData.map((_, i) => (
                    <Cell
                      key={i}
                      fill={
                        ['#3B82F6', '#22C55E', '#F59E0B', '#EF4444', '#8B5CF6', '#EC4899', '#14B8A6', '#F97316'][
                          i % 8
                        ]
                      }
                    />
                  ))}
                </Pie>
                <Legend
                  verticalAlign="bottom"
                  formatter={(value: string) => {
                    const item = subcategoryData.find((s) => s.subcategory === value);
                    const amt = item ? `$${item.amount.toLocaleString()}` : '';
                    const pct = item ? `${item.percentage.toFixed(0)}%` : '';
                    return (
                      <span className="text-xs text-[var(--color-text)]">
                        {toTitleCase(value)} {amt} ({pct})
                      </span>
                    );
                  }}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <p className="py-8 text-center text-sm text-[var(--color-text-secondary)]">
              No subcategory data available.
            </p>
          )}
        </div>
      )}

      <style>{`
        @keyframes fadeSlideIn {
          from { opacity: 0; transform: translateX(20px); }
          to   { opacity: 1; transform: translateX(0); }
        }
      `}</style>
    </div>
  );
}
