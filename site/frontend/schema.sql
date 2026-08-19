-- The subscriber list. Apply with:
--   npx wrangler d1 execute ulpia-subscribers --remote --file=./schema.sql
--
-- Deliberately four columns. Every field a list carries is a field somebody has
-- to justify later, and a pre-launch list needs to answer exactly three
-- questions: who, when, and what did they agree to.

CREATE TABLE IF NOT EXISTS subscribers (
  -- Lowercased on write, so the key does the deduplicating rather than a query.
  email           TEXT PRIMARY KEY NOT NULL,
  created_at      TEXT NOT NULL,
  -- The version of the consent sentence they accepted, so a stored consent can
  -- always be traced back to the exact words that were on screen.
  consent_version TEXT NOT NULL,
  -- Set when the person asks to be removed. Kept as a row rather than deleted so
  -- a later import cannot silently resubscribe somebody who left.
  unsubscribed_at TEXT
);

CREATE INDEX IF NOT EXISTS subscribers_created_at ON subscribers (created_at);
