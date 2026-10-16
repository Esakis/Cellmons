CREATE TABLE account_agreements (
    user_id uuid NOT NULL REFERENCES users(id),
    version text NOT NULL CHECK (char_length(version) BETWEEN 1 AND 32),
    accepted_at timestamptz NOT NULL DEFAULT now(),
    age_attested boolean NOT NULL CHECK (age_attested),
    PRIMARY KEY(user_id,version)
);
