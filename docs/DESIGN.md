# Product and Technical Design

## Product

MoveMgr organizes files by applying saved project rules. All file operations run locally. A project defines a source folder, source filters, a classification-key extractor, a target folder, a destination strategy, and a conflict policy.

## Interface

The main window shows one row per project. Users can select projects, edit folders and rules, save reusable rule tags, preview a plan, and start a run. English is the default for new installations; Korean, Japanese, and Simplified Chinese are available from the language selector in the top bar.

## Rule flow

1. Scan the source folder with the configured recursion and hidden-file options.
2. Filter by extension and filename conditions.
3. Extract a key from the filename when the destination strategy requires one.
4. Resolve the target directory.
5. Apply the conflict policy and build a read-only preview plan.
6. Execute only the approved plan.

## Safety

- Source and target roots must not overlap.
- Existing files are never overwritten.
- Same-volume moves use no-clobber semantics.
- Cross-volume moves copy to a temporary file, verify SHA-256, rename into place, and then delete the source.
- A source changed after preview is blocked.
- Cancellation occurs at safe file boundaries.
- Run journals preserve enough information to explain partial failures.

## Storage

Settings use UTF-8 JSON with an independent schema version. Portable builds store settings in `MoveMgrData` beside the executable. Exported settings can replace the current configuration or append projects.

## Architecture

- `src/`: Svelte interface, localization, client-side helpers, and IPC wrappers.
- `src-tauri/src/`: validation, planning, execution, storage, and Tauri commands.
- `scripts/`: version synchronization and portable-build tooling.
- `.github/workflows/`: verification and packaging jobs.

The application version is synchronized through `version.json`. A source change increments the patch version when the build preparation script runs.
