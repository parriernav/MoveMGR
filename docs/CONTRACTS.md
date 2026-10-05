# Data and Command Contracts

## Settings

`LocalState` contains a schema version, revision, preferences, projects, and reusable rule tags. Preferences include theme, language, and preview-before-run. Supported languages are `en`, `ko`, `ja`, and `zh`.

Each project contains:

- a stable UUID and display name;
- ordered source roots, recursion, hidden-file, extension, and filename filters;
- classification-key extraction and comparison options;
- target root and destination strategy;
- same-name conflict policy.

Settings are UTF-8 JSON. Writes use revision checks so stale windows cannot overwrite newer state. Imports validate the schema and every project before replacing or appending data.

Source folders are stored in `source.roots`, an ordered string array. Legacy `source.root` values load as a single-entry list (or an empty list for an unset folder); saves and exports use `roots`. An explicit list takes precedence over a legacy field. An empty list is allowed for an incomplete project but cannot run. Every source is checked for overlap with other sources and targets before planning. Folders are scanned in list order, with deterministic relative-path order inside each folder; conflict reservations and round-robin counters span all folders within the project. Rule tags do not include folder paths.

Source rules include `moveUnit`: `file` (default) or `sameNameGroup`. This field also belongs to source rule tags, with older settings and tags defaulting to `file`. In group mode, eligible files are identified by their source parent folder and full extensionless name normalized with NFC and the comparison ignore-case option. The first member planned to move reserves the group's destination and consumes one round-robin turn. Other members reuse that destination without advancing the counter, even when unrelated filenames appear between members in source order. Source filters, filename conflicts, execution failures, and cancellation remain per file. Unknown move units are rejected.

The `matchSubfolder` destination includes `multipleMatches`: `first`, `roundRobin`, `leastFilled`, or `skip`, presented in that order. New rules and missing values in older projects and rule tags default to `first`; explicitly saved policies are preserved and unknown policy values are rejected. Matching folders use deterministic name order. Round-robin counters are scoped to a project's plan and its ordered matching-folder group, and advance only for items planned to move. Execution retains planned destinations.

`leastFilled` chooses the folder with the smallest projected item count, breaking ties in matching-folder name order. Count regular files directly inside each folder regardless of source filters, excluding nested folders and symbolic links. With `sameNameGroup`, count distinct extensionless names normalized with NFC and the project's ignore-case setting instead of individual files. Contents are read once per folder and adjusted for every successfully planned arrival and departure across selected projects, including other destination policies; skipped items do not change counts. Bundles keep the first movable member's destination. An unreadable folder produces `TARGET_FOLDER_UNREADABLE` rather than being treated as empty. Name-only example evaluation assumes all folders are empty, so both distribution policies start with the first matching folder; the actual preview uses filesystem counts.

## Planning

A plan is immutable and identifies every candidate by project, source path, extracted key, proposed target, decision, reason code, and byte size. Planning never modifies disk contents.

Decision values are `move`, `skip`, and `blocked`. Stable reason codes are localized by the interface.

## Execution

Execution accepts a previously created plan ID. It rechecks source metadata, target availability, overlap rules, and conflicts before moving each item. Results record moved, skipped, failed, and source-retained counts plus per-item details.

## Commands

- `get_app_info`
- `load_state`
- `save_state`
- `create_plan`
- `start_run`
- `cancel_run`
- `list_history`
- `export_settings`
- `import_settings`

Commands return structured values on success and user-displayable errors on failure. The frontend localizes known plan reason codes.

## Disk layout

Portable installations keep `settings.json`, `settings.backup.json`, and run history under `MoveMgrData`. Writes use a temporary file followed by replacement to reduce corruption risk.
