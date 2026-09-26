ALTER TABLE tasks ADD COLUMN archived BOOLEAN NOT NULL DEFAULT 0;

CREATE INDEX idx_tasks_archived ON tasks(archived);
