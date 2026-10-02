# MoveMgr Manual

MoveMgr organizes files from source folders into target folders with reusable project rules.

1. Add a project and choose its source and target folders.
2. Set source rules for extensions, filename filters, subfolders, and hidden files.
3. Set target rules to extract a key from each filename and decide where matching files go.
4. Use **Preview** to check the planned moves. Files are not changed until you start the move.
5. Export your settings when you want a portable backup.

In **Source rules → Move unit**, choose **Individual files** (default) or **Files with the same name as a group**. Group mode treats `clip.mp4`, `clip.txt`, and `clip.png` in the same source folder as one bundle, using the full name without its extension. Eligible members share the destination chosen for the first member planned to move, and round-robin distribution advances once per bundle. Different names with the same classification key remain separate bundles; files in different source subfolders also form separate bundles. Grouping uses NFC normalization and the target rule's ignore-case setting.

Source filters still apply to each member, so include every extension you want to move. Existing name conflicts, execution failures, and cancellation are handled per file; grouping keeps destinations together and does not make the move a transaction. The move unit is saved with projects, source rule tags, and exported settings; older settings and tags default to individual files.

When using **Find an existing folder by key**, choose **When multiple folders match**:

- **Send all to the first folder** (default) places files in the first matching folder ordered by name.
- **Distribute evenly in turn** cycles through matching folders in name order (A, B, C, A, ...). Files are processed in source-path order, with one turn per file or filename bundle according to the source move unit. Each project plan starts a fresh rotation for each matching folder group, so equivalent keys (including ignored case and prefix comparisons) share a rotation when they match the same folders. Existing folder contents are not counted, and files skipped by the same-name policy do not consume a turn.
- **Skip** leaves files in the source when more than one folder matches.

The preview records each chosen destination; execution uses that plan. The policy is saved with projects, target rule tags, and exported settings. Older settings without this option default to **Send all to the first folder**. Explicitly saved policies are preserved.

If MoveMgr makes your file chores a little less annoying, please [give the project a GitHub star](https://github.com/parriernav/MoveMGR). It would make this tiny organizer very happy ★
