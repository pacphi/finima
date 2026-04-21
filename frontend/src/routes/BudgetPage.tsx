import { useState, useEffect, useCallback, useMemo } from 'react';
import { useApi } from '@/hooks/useApi';
import { createBudgetApi } from '@/api/budgets';
import { createSavingsGoalApi } from '@/api/savingsGoals';
import { usePortfolioStore } from '@/stores/portfolioStore';
import { formatCurrencyCompact as formatCurrency, toTitleCase } from '@/utils/format';
import { BudgetProgress } from '@/components/charts/BudgetProgress';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { Budget, BudgetVsActual, BudgetSuggestion, SavingsGoal } from '@/types/models';

function formatMonthDisplay(month: string): string {
  const d = new Date(month + '-01');
  return d.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
}

function shiftMonth(month: string, delta: number): string {
  const d = new Date(month + '-01');
  d.setMonth(d.getMonth() + delta);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
}

function getCurrentMonth(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
}

export function BudgetPage() {
  const api = useApi();
  const budgetApi = useMemo(() => createBudgetApi(api), [api]);
  const goalsApi = useMemo(() => createSavingsGoalApi(api), [api]);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);

  const [month, setMonth] = useState(getCurrentMonth);
  const [budgetData, setBudgetData] = useState<BudgetVsActual[]>([]);
  const [budgets, setBudgets] = useState<Budget[]>([]);
  const [goals, setGoals] = useState<SavingsGoal[]>([]);
  const [suggestions, setSuggestions] = useState<BudgetSuggestion[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [loading, setLoading] = useState(true);

  // Inline editing state
  const [editingCategory, setEditingCategory] = useState<string | null>(null);
  const [editLimit, setEditLimit] = useState('');

  // New budget form
  const [showNewBudget, setShowNewBudget] = useState(false);
  const [newCategory, setNewCategory] = useState('');
  const [newLimit, setNewLimit] = useState('');

  // New goal modal
  const [showGoalModal, setShowGoalModal] = useState(false);
  const [goalName, setGoalName] = useState('');
  const [goalTarget, setGoalTarget] = useState('');
  const [goalDate, setGoalDate] = useState('');

  const loadData = useCallback(async () => {
    try {
      const [budgetRes, budgetListRes, goalsRes] = await Promise.allSettled([
        budgetApi.getBudgetVsActual(month, activePortfolioId),
        budgetApi.listBudgets(month, activePortfolioId),
        goalsApi.listGoals(activePortfolioId),
      ]);
      if (budgetRes.status === 'fulfilled') setBudgetData(budgetRes.value);
      if (budgetListRes.status === 'fulfilled') setBudgets(budgetListRes.value);
      if (goalsRes.status === 'fulfilled') setGoals(goalsRes.value);
    } catch {
      // handled by empty state
    } finally {
      setLoading(false);
    }
  }, [budgetApi, goalsApi, month, activePortfolioId]);

  useEffect(() => {
    let ignore = false;
    (async () => {
      try {
        const [budgetRes, budgetListRes, goalsRes] = await Promise.allSettled([
          budgetApi.getBudgetVsActual(month, activePortfolioId),
          budgetApi.listBudgets(month, activePortfolioId),
          goalsApi.listGoals(activePortfolioId),
        ]);
        if (ignore) return;
        if (budgetRes.status === 'fulfilled') setBudgetData(budgetRes.value);
        if (budgetListRes.status === 'fulfilled') setBudgets(budgetListRes.value);
        if (goalsRes.status === 'fulfilled') setGoals(goalsRes.value);
      } finally {
        if (!ignore) setLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [budgetApi, goalsApi, month, activePortfolioId]);

  const handleAutoSuggest = useCallback(async () => {
    try {
      const result = await budgetApi.autoSuggestBudgets(activePortfolioId);
      setSuggestions(result);
      setShowSuggestions(true);
    } catch {
      // ignore
    }
  }, [budgetApi, activePortfolioId]);

  const handleApplySuggestion = useCallback(
    async (suggestion: BudgetSuggestion) => {
      try {
        await budgetApi.createBudget({
          portfolio_id: activePortfolioId,
          category: suggestion.category,
          amount: suggestion.suggested_limit,
          month,
        });
        await loadData();
        setSuggestions((prev) => prev.filter((s) => s.category !== suggestion.category));
      } catch {
        // ignore
      }
    },
    [budgetApi, month, loadData, activePortfolioId],
  );

  const handleSaveEdit = useCallback(
    async (category: string) => {
      const val = parseFloat(editLimit);
      if (isNaN(val)) return;
      try {
        const existing = budgets.find((b) => b.category === category);
        if (existing) {
          await budgetApi.updateBudget(existing.id, { amount: val });
        } else {
          await budgetApi.createBudget({
            portfolio_id: activePortfolioId,
            category,
            amount: val,
            month,
          });
        }
        setEditingCategory(null);
        await loadData();
      } catch {
        // ignore
      }
    },
    [editLimit, budgetApi, budgets, month, loadData, activePortfolioId],
  );

  const handleCreateBudget = useCallback(async () => {
    const val = parseFloat(newLimit);
    if (!newCategory.trim() || isNaN(val)) return;
    try {
      await budgetApi.createBudget({
        portfolio_id: activePortfolioId,
        category: newCategory.trim(),
        amount: val,
        month,
      });
      setShowNewBudget(false);
      setNewCategory('');
      setNewLimit('');
      await loadData();
    } catch {
      // ignore
    }
  }, [newCategory, newLimit, budgetApi, month, loadData, activePortfolioId]);

  const handleCreateGoal = useCallback(async () => {
    const target = parseFloat(goalTarget);
    if (!goalName.trim() || isNaN(target)) return;
    try {
      await goalsApi.createGoal({
        name: goalName.trim(),
        target_amount: target,
        target_date: goalDate || undefined,
        portfolio_id: activePortfolioId,
      });
      setShowGoalModal(false);
      setGoalName('');
      setGoalTarget('');
      setGoalDate('');
      await loadData();
    } catch {
      // ignore
    }
  }, [goalName, goalTarget, goalDate, goalsApi, loadData, activePortfolioId]);

  const budgetGoalModalRef = useFocusTrap<HTMLDivElement>(showGoalModal, () =>
    setShowGoalModal(false),
  );

  const totalBudget = budgetData.reduce((s, b) => s + b.limit, 0);
  const totalSpent = budgetData.reduce((s, b) => s + b.spent, 0);
  const totalRemaining = totalBudget - totalSpent;
  const totalPct = totalBudget > 0 ? (totalSpent / totalBudget) * 100 : 0;

  return (
    <div className="p-6">
      {/* Header with month nav */}
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold text-[var(--color-text)]">Budget</h1>
        <div className="flex items-center gap-3" role="group" aria-label="Month navigation">
          <button
            onClick={() => setMonth((m) => shiftMonth(m, -1))}
            aria-label="Previous month"
            className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
          >
            Prev
          </button>
          <span className="text-sm font-medium text-[var(--color-text)]" aria-live="polite">
            {formatMonthDisplay(month)}
          </span>
          <button
            onClick={() => setMonth((m) => shiftMonth(m, 1))}
            aria-label="Next month"
            className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
          >
            Next
          </button>
        </div>
      </div>

      {/* Auto suggest button */}
      <div className="mb-4 flex gap-3">
        <button
          onClick={handleAutoSuggest}
          className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
        >
          Auto-Suggest Budget
        </button>
        <button
          onClick={() => setShowNewBudget(true)}
          className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm font-medium text-[var(--color-text)] hover:bg-[var(--color-bg-secondary)]"
        >
          + New Budget Entry
        </button>
      </div>

      {/* Suggestions */}
      {showSuggestions && suggestions.length > 0 && (
        <div className="mb-6 rounded-lg border border-[var(--color-accent)] bg-[var(--color-bg)] p-4">
          <h3 className="mb-3 text-sm font-semibold text-[var(--color-text)]">
            Suggested Budgets (based on 3-month average)
          </h3>
          <div className="space-y-2">
            {suggestions.map((s) => (
              <div key={s.category} className="flex items-center justify-between">
                <div>
                  <span className="text-sm text-[var(--color-text)]">
                    {toTitleCase(s.category)}
                  </span>
                  <span className="ml-2 text-xs text-[var(--color-text-secondary)]">
                    Avg: {formatCurrency(s.avg_monthly)} → Suggested:{' '}
                    {formatCurrency(s.suggested_limit)}
                  </span>
                </div>
                <button
                  onClick={() => void handleApplySuggestion(s)}
                  className="rounded-md bg-[var(--color-accent)] px-3 py-1 text-xs font-medium text-white hover:opacity-90"
                >
                  Apply
                </button>
              </div>
            ))}
          </div>
          <button
            onClick={() => setShowSuggestions(false)}
            className="mt-3 text-xs text-[var(--color-text-secondary)] underline"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* New budget form */}
      {showNewBudget && (
        <div className="mb-6 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
          <h3 className="mb-3 text-sm font-semibold text-[var(--color-text)]">New Budget Entry</h3>
          <div className="flex gap-3">
            <div>
              <label htmlFor="new-budget-category" className="sr-only">
                Category
              </label>
              <input
                id="new-budget-category"
                type="text"
                placeholder="Category"
                value={newCategory}
                onChange={(e) => setNewCategory(e.target.value)}
                aria-required="true"
                className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
              />
            </div>
            <div>
              <label htmlFor="new-budget-limit" className="sr-only">
                Budget limit
              </label>
              <input
                id="new-budget-limit"
                type="number"
                placeholder="Limit"
                value={newLimit}
                onChange={(e) => setNewLimit(e.target.value)}
                aria-required="true"
                className="w-32 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
              />
            </div>
            <button
              onClick={() => void handleCreateBudget()}
              className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
            >
              Create
            </button>
            <button
              onClick={() => setShowNewBudget(false)}
              className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-[var(--color-text)]"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Budget table */}
      {loading ? (
        <p className="text-sm text-[var(--color-text-secondary)]">Loading...</p>
      ) : (
        <div className="mb-8 overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)]">
          <table className="w-full text-sm" aria-label={`Budget for ${formatMonthDisplay(month)}`}>
            <thead>
              <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
                <th className="px-4 py-3 text-left font-medium text-[var(--color-text-secondary)]">
                  Category
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Budget
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Spent
                </th>
                <th className="px-4 py-3 text-right font-medium text-[var(--color-text-secondary)]">
                  Remaining
                </th>
                <th className="w-64 px-4 py-3 font-medium text-[var(--color-text-secondary)]">
                  Progress
                </th>
                <th className="px-4 py-3 font-medium text-[var(--color-text-secondary)]">Action</th>
              </tr>
            </thead>
            <tbody>
              {budgetData.map((b) => (
                <tr key={b.category} className="border-b border-[var(--color-border)]">
                  <td className="px-4 py-3 text-[var(--color-text)]">{toTitleCase(b.category)}</td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {editingCategory === b.category ? (
                      <input
                        type="number"
                        value={editLimit}
                        onChange={(e) => setEditLimit(e.target.value)}
                        aria-label={`Budget limit for ${b.category}`}
                        className="w-24 rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-right text-sm"
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') void handleSaveEdit(b.category);
                          if (e.key === 'Escape') setEditingCategory(null);
                        }}
                      />
                    ) : (
                      formatCurrency(b.limit)
                    )}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(b.spent)}
                  </td>
                  <td
                    className={`px-4 py-3 text-right ${b.remaining < 0 ? 'text-red-500' : 'text-[var(--color-text)]'}`}
                  >
                    {formatCurrency(b.remaining)}
                  </td>
                  <td className="px-4 py-3">
                    <BudgetProgress data={b} />
                  </td>
                  <td className="px-4 py-3">
                    {editingCategory === b.category ? (
                      <div className="flex gap-1">
                        <button
                          onClick={() => void handleSaveEdit(b.category)}
                          className="text-xs text-[var(--color-accent)] hover:underline"
                        >
                          Save
                        </button>
                        <button
                          onClick={() => setEditingCategory(null)}
                          className="text-xs text-[var(--color-text-secondary)] hover:underline"
                        >
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <button
                        onClick={() => {
                          setEditingCategory(b.category);
                          setEditLimit(String(b.limit));
                        }}
                        className="text-xs text-[var(--color-accent)] hover:underline"
                      >
                        Edit
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {budgetData.length > 0 && (
                <tr className="bg-[var(--color-bg-secondary)] font-medium">
                  <td className="px-4 py-3 text-[var(--color-text)]">TOTAL</td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(totalBudget)}
                  </td>
                  <td className="px-4 py-3 text-right text-[var(--color-text)]">
                    {formatCurrency(totalSpent)}
                  </td>
                  <td
                    className={`px-4 py-3 text-right ${totalRemaining < 0 ? 'text-red-500' : 'text-[var(--color-text)]'}`}
                  >
                    {formatCurrency(totalRemaining)}
                  </td>
                  <td className="px-4 py-3 text-sm text-[var(--color-text-secondary)]">
                    {totalPct.toFixed(0)}%
                  </td>
                  <td />
                </tr>
              )}
            </tbody>
          </table>
          {budgetData.length === 0 && (
            <div className="p-8 text-center text-sm text-[var(--color-text-secondary)]">
              No budgets set for this month. Use "Auto-Suggest" or create one manually.
            </div>
          )}
        </div>
      )}

      {/* Savings Goals */}
      <div>
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-[var(--color-text)]">Savings Goals</h2>
          <button
            onClick={() => setShowGoalModal(true)}
            className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
          >
            + New Goal
          </button>
        </div>

        {goals.length > 0 ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {goals.map((goal) => {
              const pct =
                goal.target_amount > 0 ? (goal.current_amount / goal.target_amount) * 100 : 0;
              return (
                <div
                  key={goal.id}
                  className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4"
                >
                  <h3 className="text-sm font-semibold text-[var(--color-text)]">{goal.name}</h3>
                  <div className="mt-2 flex items-baseline justify-between">
                    <span className="text-lg font-bold text-[var(--color-text)]">
                      {formatCurrency(goal.current_amount)}
                    </span>
                    <span className="text-sm text-[var(--color-text-secondary)]">
                      / {formatCurrency(goal.target_amount)}
                    </span>
                  </div>
                  <div
                    className="mt-2 h-2.5 w-full overflow-hidden rounded-full bg-[var(--color-border)]"
                    role="progressbar"
                    aria-valuenow={Math.round(pct)}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label={`${goal.name}: ${pct.toFixed(0)}% complete`}
                  >
                    <div
                      className="h-full rounded-full bg-[var(--color-accent)] transition-all"
                      style={{ width: `${Math.min(pct, 100)}%` }}
                    />
                  </div>
                  <div className="mt-1 flex justify-between text-xs text-[var(--color-text-secondary)]">
                    <span>{pct.toFixed(0)}%</span>
                    {goal.target_date && (
                      <span>
                        Target:{' '}
                        {new Date(goal.target_date).toLocaleDateString('en-US', {
                          month: 'short',
                          year: 'numeric',
                        })}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <p className="text-sm text-[var(--color-text-secondary)]">
            No savings goals yet. Create one to start tracking your progress.
          </p>
        )}
      </div>

      {/* Goal creation modal */}
      {showGoalModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div
            ref={budgetGoalModalRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby="budget-new-goal-title"
            className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-6"
            tabIndex={-1}
          >
            <h3
              id="budget-new-goal-title"
              className="mb-4 text-lg font-semibold text-[var(--color-text)]"
            >
              New Savings Goal
            </h3>
            <div className="space-y-3">
              <div>
                <label
                  htmlFor="budget-goal-name"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Goal Name
                </label>
                <input
                  id="budget-goal-name"
                  type="text"
                  value={goalName}
                  onChange={(e) => setGoalName(e.target.value)}
                  placeholder="Emergency Fund"
                  aria-required="true"
                  className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
                />
              </div>
              <div>
                <label
                  htmlFor="budget-goal-target"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Target Amount
                </label>
                <input
                  id="budget-goal-target"
                  type="number"
                  value={goalTarget}
                  onChange={(e) => setGoalTarget(e.target.value)}
                  placeholder="15000"
                  aria-required="true"
                  className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
                />
              </div>
              <div>
                <label
                  htmlFor="budget-goal-date"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Target Date (optional)
                </label>
                <input
                  id="budget-goal-date"
                  type="date"
                  value={goalDate}
                  onChange={(e) => setGoalDate(e.target.value)}
                  className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
                />
              </div>
            </div>
            <div className="mt-6 flex justify-end gap-3">
              <button
                onClick={() => setShowGoalModal(false)}
                className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-[var(--color-text)]"
              >
                Cancel
              </button>
              <button
                onClick={() => void handleCreateGoal()}
                className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
              >
                Create Goal
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
