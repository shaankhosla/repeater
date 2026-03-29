-- Add sync tracking to cards table.
ALTER TABLE cards ADD COLUMN locally_modified INTEGER NOT NULL DEFAULT 0;
