ALTER TABLE boosters DROP CONSTRAINT boosters_price_check;
ALTER TABLE boosters ADD COLUMN source text NOT NULL DEFAULT 'purchase' CHECK(source IN ('purchase','tournament'));
ALTER TABLE boosters ADD CONSTRAINT boosters_price_source_check CHECK
    ((source='purchase' AND price>0) OR (source='tournament' AND price=0));
CREATE INDEX reports_reporter_history_idx ON moderation_reports(reporter,created_at DESC);
CREATE INDEX users_ban_expiry_idx ON users(banned_until) WHERE status='banned' AND banned_until IS NOT NULL;
CREATE INDEX filters_enabled_idx ON word_filters(enabled) WHERE enabled;
