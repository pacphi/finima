-- 019_user_overrides_unique.sql
-- The override repo uses ON CONFLICT (user_id, description_pattern) but the
-- original migration omitted the UNIQUE constraint.  Add it now.

-- Remove any duplicates first (keep the newest by id).
DELETE FROM user_category_overrides a
USING user_category_overrides b
WHERE a.user_id = b.user_id
  AND a.description_pattern = b.description_pattern
  AND a.id < b.id;

CREATE UNIQUE INDEX idx_user_overrides_user_pattern
    ON user_category_overrides (user_id, description_pattern);
