-- R-RNK-01/05: account rating starts at 1000 and never drops below 1.
CREATE TABLE ratings (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    rating integer NOT NULL DEFAULT 1000 CHECK (rating >= 1),
    games integer NOT NULL DEFAULT 0 CHECK (games >= 0),
    wins integer NOT NULL DEFAULT 0 CHECK (wins >= 0),
    draws integer NOT NULL DEFAULT 0 CHECK (draws >= 0),
    losses integer NOT NULL DEFAULT 0 CHECK (losses >= 0),
    disconnects integer NOT NULL DEFAULT 0 CHECK (disconnects >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT ratings_results_total CHECK (games::bigint = wins::bigint + draws::bigint + losses::bigint)
);
CREATE INDEX ratings_ranking_idx ON ratings(rating DESC, user_id);
