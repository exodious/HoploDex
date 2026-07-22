-- Seed the initial FirearmType set per spec.md's Assumptions and
-- research.md §10 (generic per-type thumbnails).

INSERT INTO firearm_types (name, generic_thumbnail_key) VALUES
    ('Handgun', 'handgun'),
    ('Rifle', 'rifle'),
    ('Shotgun', 'shotgun'),
    ('Other', 'other');
