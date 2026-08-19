PRAGMA foreign_keys = ON;

UPDATE items
SET status = 'committed'
WHERE status = 'abandoned'
  AND run_id IN (
      SELECT id
      FROM runs
      WHERE status IN ('failed', 'cancelled')
  );
