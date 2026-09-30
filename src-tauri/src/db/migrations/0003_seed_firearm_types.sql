-- Seeds both lookup lists: the initial FirearmType set per spec.md's
-- Assumptions and research.md §10 (generic per-type thumbnails), and
-- specs/004-cartridges-action-types' action types with their mapping to the
-- seeded types (FR-018; research.md §10). The file keeps its name because
-- `schema_migrations` records it.

INSERT INTO firearm_types (name, generic_thumbnail_key) VALUES
    ('Handgun', 'handgun'),
    ('Rifle', 'rifle'),
    ('Shotgun', 'shotgun'),
    ('Other', 'other');

-- Fixed ids, so an id means the same action in every build; the list's
-- order is `sort_order`.
INSERT INTO action_types (id, name, sort_order) VALUES
    (1, 'Semi-automatic', 1),
    (2, 'Revolver', 2),
    (3, 'Bolt action', 3),
    (4, 'Lever action', 4),
    (5, 'Pump action', 5),
    (6, 'Break action', 6),
    (7, 'Falling block', 7),
    (8, 'Rolling block', 8),
    (9, 'Single shot (other)', 9),
    (10, 'Flintlock', 10),
    (11, 'Percussion', 11),
    (12, 'Inline muzzleloader', 12);

-- FR-018's table. Other (4) has no rows, so it allows every action (FR-017).
-- Handgun (1): all but Pump action, Falling block and Inline muzzleloader.
INSERT INTO firearm_type_actions (firearm_type_id, action_type_id)
SELECT 1, id FROM action_types WHERE id NOT IN (5, 7, 12);
-- Rifle (2): all.
INSERT INTO firearm_type_actions (firearm_type_id, action_type_id)
SELECT 2, id FROM action_types;
-- Shotgun (3): all but Rolling block.
INSERT INTO firearm_type_actions (firearm_type_id, action_type_id)
SELECT 3, id FROM action_types WHERE id <> 8;
