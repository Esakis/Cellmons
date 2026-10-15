-- Results retain their original balances when a notification is retried later.
ALTER TABLE matches
    ADD COLUMN first_rating_new integer NOT NULL DEFAULT 1000 CHECK (first_rating_new >= 1),
    ADD COLUMN second_rating_new integer NOT NULL DEFAULT 1000 CHECK (second_rating_new >= 1);
UPDATE matches m SET first_rating_new=h.after FROM rating_history h
WHERE h.match_id=m.id AND h.user_id=m.first_user;
UPDATE matches m SET second_rating_new=h.after FROM rating_history h
WHERE h.match_id=m.id AND h.user_id=m.second_user;
