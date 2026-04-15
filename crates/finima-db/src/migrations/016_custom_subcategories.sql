-- 016_custom_subcategories.sql
-- Up
ALTER TABLE custom_categories ADD COLUMN parent_key TEXT;
CREATE INDEX idx_custom_categories_parent ON custom_categories (user_id, parent_key);

-- Down
-- DROP INDEX IF EXISTS idx_custom_categories_parent;
-- ALTER TABLE custom_categories DROP COLUMN parent_key;
