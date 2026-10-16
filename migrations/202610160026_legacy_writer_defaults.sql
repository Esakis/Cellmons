-- Retain compatibility with the last pre-F14 writer throughout expand deployments.
-- Current writers bind both versions explicitly. Removal belongs to a later contract release.
ALTER TABLE tournaments ALTER COLUMN engine_version SET DEFAULT '0.2.0',
    ALTER COLUMN content_version SET DEFAULT decode('2fe5309b47546cda1df29067283d34e881f6c61e58be132e127cc5dfb8225701','hex');
ALTER TABLE expeditions ALTER COLUMN engine_version SET DEFAULT '0.2.0';
