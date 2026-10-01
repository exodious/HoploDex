-- Seeds the type, action, classification and accessory-kind lists: the initial FirearmType set per spec.md's
-- Assumptions and research.md §10 (generic per-type thumbnails), and
-- specs/004-cartridges-action-types' action types with their mapping to the
-- seeded types (FR-018; research.md §10), and specs/005-regulated-item-types'
-- Suppressor type and registration classifications, and
-- specs/006-accessory-links' accessory kinds. The file keeps its name because
-- `schema_migrations` records it.

-- Fixed ids (005 research.md §4), listed in `sort_order`, which puts Other,
-- the catch-all, last. Suppressor has no action, barrel length or capacity
-- (FR-003): its three flags are 0. Its caliber is its bore, never worked out
-- from its rated cartridge (FR-002, research.md §15).
INSERT INTO firearm_types (id, name, generic_thumbnail_key, sort_order, action_type_applies, barrel_length_applies, capacity_applies, caliber_from_cartridge) VALUES
    (1, 'Handgun', 'handgun', 1, 1, 1, 1, 1),
    (2, 'Rifle', 'rifle', 2, 1, 1, 1, 1),
    (3, 'Shotgun', 'shotgun', 3, 1, 1, 1, 1),
    (4, 'Other', 'other', 5, 1, 1, 1, 1),
    (5, 'Suppressor', 'suppressor', 4, 0, 0, 0, 0);

-- FR-007: fixed ids, listed in `sort_order`, all offered. No rule about what
-- is regulated lives here.
INSERT INTO registration_classes (id, name, sort_order, offered) VALUES
    (1, 'Suppressor', 1, 1),
    (2, 'Short-barreled rifle', 2, 1),
    (3, 'Short-barreled shotgun', 3, 1),
    (4, 'Any other weapon', 4, 1),
    (5, 'Machine gun', 5, 1),
    (6, 'Destructive device', 6, 1);

-- Fixed ids, so an id means the same action in every build; the list's
-- order is `sort_order`.
INSERT INTO action_types (id, name, sort_order) VALUES
    (1, 'Semi-automatic', 1),
    (2, 'Revolver', 2),
    (3, 'Bolt action', 3),
    (4, 'Lever action', 4),
    (5, 'Pump action', 5),
    (6, 'Break action', 6),
    (13, 'Automatic or select-fire', 7),
    (7, 'Falling block', 8),
    (8, 'Rolling block', 9),
    (9, 'Single shot (other)', 10),
    (10, 'Flintlock', 11),
    (11, 'Percussion', 12),
    (12, 'Inline muzzleloader', 13);

-- FR-018's table, plus 005 US3's id 13 (Handgun, Rifle and Shotgun all allow
-- it: the exclusions below name the ids they refuse, so it is included).
-- Other (4) has no rows, so it allows every action (FR-017).
-- Handgun (1): all but Pump action, Falling block and Inline muzzleloader.
INSERT INTO firearm_type_actions (firearm_type_id, action_type_id)
SELECT 1, id FROM action_types WHERE id NOT IN (5, 7, 12);
-- Rifle (2): all.
INSERT INTO firearm_type_actions (firearm_type_id, action_type_id)
SELECT 2, id FROM action_types;
-- Shotgun (3): all but Rolling block.
INSERT INTO firearm_type_actions (firearm_type_id, action_type_id)
SELECT 3, id FROM action_types WHERE id <> 8;

-- specs/006-accessory-links FR-002 (data-model.md's "Entity: Accessory
-- Kind"): fixed ids, so an id means the same kind in every build; listed in
-- `sort_order` (equal to the id), all offered. There is no Suppressor kind.
INSERT INTO accessory_kinds (id, name, generic_thumbnail_key, sort_order, offered) VALUES
    (1, 'Optic', 'optic', 1, 1),
    (2, 'Light or laser', 'light', 2, 1),
    (3, 'Magazine', 'magazine', 3, 1),
    (4, 'Stock or brace', 'stock', 4, 1),
    (5, 'Upper receiver', 'upper', 5, 1),
    (6, 'Barrel', 'barrel', 6, 1),
    (7, 'Muzzle device', 'muzzle', 7, 1),
    (8, 'Conversion kit', 'conversion', 8, 1),
    (9, 'Mount or rail', 'mount', 9, 1),
    (10, 'Sling', 'sling', 10, 1),
    (11, 'Case', 'case', 11, 1),
    (12, 'Other', 'accessory', 12, 1);
