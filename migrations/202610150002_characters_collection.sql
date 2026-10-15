-- Content IDs refer to the compiled catalogue, not to duplicate SQL content tables.
CREATE TABLE unlocked_templates (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    template_id integer NOT NULL CHECK (template_id > 0),
    source text NOT NULL CHECK (source IN ('start', 'arena', 'tournament', 'expedition', 'junkyard', 'admin')),
    unlocked_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, template_id)
);

CREATE TABLE avatars (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slot smallint NOT NULL CHECK (slot BETWEEN 0 AND 9),
    template_id integer NOT NULL,
    path_id integer CHECK (path_id > 0),
    specialization text CHECK (specialization IN ('combat', 'divine', 'element', 'arcane', 'necro', 'tech', 'essence', 'concentration')),
    gift_id integer CHECK (gift_id > 0),
    tier smallint NOT NULL DEFAULT 0 CHECK (tier BETWEEN 0 AND 3),
    is_shade boolean NOT NULL DEFAULT false,
    shade_school text CHECK (shade_school IN ('combat', 'divine', 'element', 'arcane', 'necro', 'tech')),
    level smallint NOT NULL DEFAULT 1 CHECK (level BETWEEN 1 AND 20),
    xp integer NOT NULL DEFAULT 0 CHECK (xp >= 0),
    dev_points_unspent smallint NOT NULL DEFAULT 0 CHECK (dev_points_unspent >= 0),
    dp_phys_atk smallint NOT NULL DEFAULT 0 CHECK (dp_phys_atk >= 0),
    dp_phys_def smallint NOT NULL DEFAULT 0 CHECK (dp_phys_def >= 0),
    dp_mag_atk smallint NOT NULL DEFAULT 0 CHECK (dp_mag_atk >= 0),
    dp_mag_def smallint NOT NULL DEFAULT 0 CHECK (dp_mag_def >= 0),
    dp_hp smallint NOT NULL DEFAULT 0 CHECK (dp_hp >= 0),
    dp_stamina smallint NOT NULL DEFAULT 0 CHECK (dp_stamina >= 0),
    is_active boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT avatars_unlocked_template_fk FOREIGN KEY (user_id, template_id)
        REFERENCES unlocked_templates(user_id, template_id),
    CONSTRAINT avatars_shade_school CHECK (is_shade = (shade_school IS NOT NULL)),
    CONSTRAINT avatars_live_active CHECK (NOT is_active OR deleted_at IS NULL),
    CONSTRAINT avatars_path_tier CHECK (path_id IS NULL OR tier >= 1),
    CONSTRAINT avatars_specialization_tier CHECK (specialization IS NULL OR tier >= 2),
    CONSTRAINT avatars_gift_tier CHECK (gift_id IS NULL OR tier = 3)
);
-- R-PUL-01: deleted avatars remain in history but no longer occupy a slot.
CREATE UNIQUE INDEX avatars_live_slot_idx ON avatars(user_id, slot) WHERE deleted_at IS NULL;
-- At most one active avatar. Provisioning/switching must ensure at least one in a transaction.
CREATE UNIQUE INDEX avatars_live_active_idx ON avatars(user_id) WHERE is_active AND deleted_at IS NULL;
CREATE INDEX avatars_user_id_idx ON avatars(user_id);

CREATE TABLE collection (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    card_id smallint NOT NULL CHECK (card_id > 0),
    qty integer NOT NULL CHECK (qty >= 0),
    PRIMARY KEY (user_id, card_id)
);

CREATE TABLE decks (
    avatar_id uuid PRIMARY KEY REFERENCES avatars(id) ON DELETE CASCADE,
    cards smallint[] NOT NULL DEFAULT '{}',
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT decks_size CHECK (cardinality(cards) <= 20),
    CONSTRAINT decks_shape CHECK (cardinality(cards) = 0 OR (array_ndims(cards) = 1 AND array_lower(cards, 1) = 1)),
    CONSTRAINT decks_no_nulls CHECK (CASE WHEN array_ndims(cards) > 1 THEN false ELSE array_position(cards, NULL) IS NULL END),
    CONSTRAINT decks_positive_ids CHECK (0 < ALL(cards))
);

CREATE TABLE deck_presets (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name text NOT NULL CHECK (char_length(btrim(name)) > 0),
    cards smallint[] NOT NULL DEFAULT '{}',
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, name),
    CONSTRAINT deck_presets_size CHECK (cardinality(cards) <= 20),
    CONSTRAINT deck_presets_shape CHECK (cardinality(cards) = 0 OR (array_ndims(cards) = 1 AND array_lower(cards, 1) = 1)),
    CONSTRAINT deck_presets_no_nulls CHECK (CASE WHEN array_ndims(cards) > 1 THEN false ELSE array_position(cards, NULL) IS NULL END),
    CONSTRAINT deck_presets_positive_ids CHECK (0 < ALL(cards))
);
