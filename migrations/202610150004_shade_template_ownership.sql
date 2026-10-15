-- R-PUL-02 applies to ordinary templates; R-CIE-01 creates shades by sacrifice,
-- without adding a reusable template to unlocked_templates (R-CIE-05).
ALTER TABLE avatars DROP CONSTRAINT avatars_unlocked_template_fk;
ALTER TABLE avatars ADD CONSTRAINT avatars_template_positive CHECK (template_id > 0);
ALTER TABLE avatars ADD COLUMN owned_template_id integer
    GENERATED ALWAYS AS (CASE WHEN is_shade THEN NULL ELSE template_id END) STORED;
ALTER TABLE avatars ADD CONSTRAINT avatars_unlocked_template_fk
    FOREIGN KEY (user_id, owned_template_id) REFERENCES unlocked_templates(user_id, template_id);
