CREATE TABLE activity_locks (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    kind text NOT NULL CHECK(kind IN ('trade','tournament','expedition')),
    owner_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX activity_locks_owner_idx ON activity_locks(kind,owner_id);
ALTER TABLE boosters ADD CONSTRAINT boosters_open_state CHECK(
    (status='unopened' AND contents IS NULL AND opened_at IS NULL) OR
    (status='opened' AND contents IS NOT NULL AND opened_at IS NOT NULL)
);
ALTER TABLE boosters ADD CONSTRAINT boosters_contents_shape CHECK(
    contents IS NULL OR (array_ndims(contents)=1 AND array_lower(contents,1)=1)
);
ALTER TABLE boosters ADD CONSTRAINT boosters_contents_no_nulls CHECK(
    CASE WHEN array_ndims(contents)>1 THEN false ELSE array_position(contents,NULL) IS NULL END
);
ALTER TABLE idempotency_keys ADD CONSTRAINT idempotency_operation CHECK(operation IN ('buy','open','discard'));
