import { useState, useEffect, useCallback, useMemo } from 'react';
import { useApi } from '@/hooks/useApi';
import { createSavingsGoalApi } from '@/api/savingsGoals';
import { formatCurrencyCompact as formatCurrency } from '@/utils/format';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import type { SavingsGoal } from '@/types/models';

export function GoalsPage() {
  const api = useApi();
  const goalsApi = useMemo(() => createSavingsGoalApi(api), [api]);

  const [goals, setGoals] = useState<SavingsGoal[]>([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editingGoal, setEditingGoal] = useState<SavingsGoal | null>(null);
  const [goalName, setGoalName] = useState('');
  const [goalTarget, setGoalTarget] = useState('');
  const [goalDate, setGoalDate] = useState('');
  const [goalContribution, setGoalContribution] = useState('');

  const loadGoals = useCallback(async () => {
    setLoading(true);
    try {
      const data = await goalsApi.listGoals();
      setGoals(data);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [goalsApi]);

  useEffect(() => {
    void loadGoals();
  }, [loadGoals]);

  const handleCreate = useCallback(async () => {
    const target = parseFloat(goalTarget);
    if (!goalName.trim() || isNaN(target)) return;
    try {
      if (editingGoal) {
        await goalsApi.updateGoal(editingGoal.id, {
          name: goalName.trim(),
          target_amount: target,
          target_date: goalDate || undefined,
          monthly_contribution: goalContribution ? parseFloat(goalContribution) : undefined,
        });
      } else {
        await goalsApi.createGoal({
          name: goalName.trim(),
          target_amount: target,
          target_date: goalDate || undefined,
          monthly_contribution: goalContribution ? parseFloat(goalContribution) : undefined,
        });
      }
      setShowModal(false);
      setEditingGoal(null);
      setGoalName('');
      setGoalTarget('');
      setGoalDate('');
      setGoalContribution('');
      await loadGoals();
    } catch {
      // ignore
    }
  }, [goalName, goalTarget, goalDate, goalContribution, editingGoal, goalsApi, loadGoals]);

  const handleOpenEdit = (goal: SavingsGoal) => {
    setEditingGoal(goal);
    setGoalName(goal.name);
    setGoalTarget(String(goal.target_amount));
    setGoalDate(goal.target_date ?? '');
    setGoalContribution(goal.monthly_contribution ? String(goal.monthly_contribution) : '');
    setShowModal(true);
  };

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await goalsApi.deleteGoal(id);
        await loadGoals();
      } catch {
        // ignore
      }
    },
    [goalsApi, loadGoals],
  );

  const goalModalRef = useFocusTrap<HTMLDivElement>(showModal, () => setShowModal(false));

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center p-6">
        <span className="text-[var(--color-text-secondary)]">Loading goals...</span>
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold text-[var(--color-text)]">Savings Goals</h1>
        <button
          onClick={() => {
            setEditingGoal(null);
            setGoalName('');
            setGoalTarget('');
            setGoalDate('');
            setGoalContribution('');
            setShowModal(true);
          }}
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
            const remaining = goal.target_amount - goal.current_amount;
            const monthsLeft =
              goal.monthly_contribution > 0 && remaining > 0
                ? Math.ceil(remaining / goal.monthly_contribution)
                : null;

            return (
              <div
                key={goal.id}
                className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5"
              >
                <div className="flex items-start justify-between">
                  <h3 className="text-base font-semibold text-[var(--color-text)]">{goal.name}</h3>
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleOpenEdit(goal)}
                      className="text-xs text-[var(--color-accent)] hover:underline"
                      aria-label={`Edit goal: ${goal.name}`}
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => void handleDelete(goal.id)}
                      className="text-xs text-[var(--color-text-secondary)] hover:text-red-500"
                      aria-label={`Delete goal: ${goal.name}`}
                    >
                      Delete
                    </button>
                  </div>
                </div>
                <div className="mt-3 flex items-baseline justify-between">
                  <span className="text-xl font-bold text-[var(--color-text)]">
                    {formatCurrency(goal.current_amount)}
                  </span>
                  <span className="text-sm text-[var(--color-text-secondary)]">
                    / {formatCurrency(goal.target_amount)}
                  </span>
                </div>
                <div
                  className="mt-3 h-3 w-full overflow-hidden rounded-full bg-[var(--color-border)]"
                  role="progressbar"
                  aria-valuenow={Math.round(pct)}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label={`${goal.name}: ${pct.toFixed(0)}% complete, ${formatCurrency(goal.current_amount)} of ${formatCurrency(goal.target_amount)}`}
                >
                  <div
                    className="h-full rounded-full bg-[var(--color-accent)] transition-all"
                    style={{ width: `${Math.min(pct, 100)}%` }}
                  />
                </div>
                <div className="mt-2 flex justify-between text-xs text-[var(--color-text-secondary)]">
                  <span>{pct.toFixed(0)}% complete</span>
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
                {monthsLeft !== null && (
                  <p className="mt-2 text-xs text-[var(--color-text-secondary)]">
                    ~{monthsLeft} months to completion at{' '}
                    {formatCurrency(goal.monthly_contribution)}/mo
                  </p>
                )}
              </div>
            );
          })}
        </div>
      ) : (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-12 text-center">
          <p className="text-[var(--color-text-secondary)]">
            No savings goals yet. Create one to start tracking your progress.
          </p>
        </div>
      )}

      {/* Goal creation modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div
            ref={goalModalRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby="new-goal-title"
            className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-6"
            tabIndex={-1}
          >
            <h3 id="new-goal-title" className="mb-4 text-lg font-semibold text-[var(--color-text)]">
              {editingGoal ? 'Edit Savings Goal' : 'New Savings Goal'}
            </h3>
            <div className="space-y-3">
              <div>
                <label
                  htmlFor="goal-name"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Goal Name
                </label>
                <input
                  id="goal-name"
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
                  htmlFor="goal-target"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Target Amount
                </label>
                <input
                  id="goal-target"
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
                  htmlFor="goal-date"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Target Date (optional)
                </label>
                <input
                  id="goal-date"
                  type="date"
                  value={goalDate}
                  onChange={(e) => setGoalDate(e.target.value)}
                  className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
                />
              </div>
              <div>
                <label
                  htmlFor="goal-contribution"
                  className="mb-1 block text-sm text-[var(--color-text-secondary)]"
                >
                  Monthly Contribution (optional)
                </label>
                <input
                  id="goal-contribution"
                  type="number"
                  value={goalContribution}
                  onChange={(e) => setGoalContribution(e.target.value)}
                  placeholder="500"
                  className="w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)]"
                />
              </div>
            </div>
            <div className="mt-6 flex justify-end gap-3">
              <button
                onClick={() => setShowModal(false)}
                className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-[var(--color-text)]"
              >
                Cancel
              </button>
              <button
                onClick={() => void handleCreate()}
                className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90"
              >
                {editingGoal ? 'Update Goal' : 'Create Goal'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
