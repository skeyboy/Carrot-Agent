PRAGMA foreign_keys = ON;

UPDATE items
SET status = 'abandoned'
WHERE status = 'committed'
  AND run_id IN (
      SELECT id
      FROM runs
      WHERE status IN ('failed', 'cancelled')
  );
