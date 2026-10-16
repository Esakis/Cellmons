ALTER TABLE sessions ADD COLUMN purpose text NOT NULL DEFAULT 'game'
    CHECK (purpose IN ('game','privacy'));
