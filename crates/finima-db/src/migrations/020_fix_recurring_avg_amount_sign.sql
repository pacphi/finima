-- 020_fix_recurring_avg_amount_sign.sql
--
-- avg_amount was computed using ABS(amount), making all groups look like
-- income.  Recompute from the linked transactions to preserve the sign
-- (negative = expense, positive = income).

UPDATE recurring_groups rg
SET avg_amount = sub.real_avg
FROM (
    SELECT recurring_group_id,
           AVG(amount) AS real_avg
    FROM transactions
    WHERE recurring_group_id IS NOT NULL
    GROUP BY recurring_group_id
) sub
WHERE rg.id = sub.recurring_group_id
  AND sub.real_avg IS NOT NULL;

-- For any groups that have no linked transactions (edge case), negate the
-- amount since the vast majority of recurring payments are expenses.
UPDATE recurring_groups
SET avg_amount = -avg_amount
WHERE avg_amount > 0
  AND id NOT IN (
      SELECT DISTINCT recurring_group_id
      FROM transactions
      WHERE recurring_group_id IS NOT NULL
  );

-- Down
-- UPDATE recurring_groups SET avg_amount = ABS(avg_amount);
