ALTER TABLE tasks ADD COLUMN due_date TEXT;

CREATE INDEX idx_tasks_due_date ON tasks(due_date);
