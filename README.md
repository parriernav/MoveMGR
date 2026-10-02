# MoveMgr

MoveMgr is a local desktop app for organizing files from multiple source folders into target folders with reusable rules. It is built with Tauri 2, Svelte 5, TypeScript, and Rust for Windows and macOS.

- Current version: see [`version.json`](version.json)
- Interface languages: English, Korean, Japanese, and Simplified Chinese
- Default language for new installations: English
- Themes: system, light, and dark

## Features

- Create, duplicate, delete, reorder, and select multiple projects.
- Filter source files by extension, filename, subfolder recursion, and hidden-file status.
- Keep files with the same extensionless name together and distribute whole filename groups across matching folders.
- Extract classification keys from whole names, prefixes, or delimiter-separated names.
- Move files to the target root, a fixed subfolder, an existing matching folder, or a key-named folder.
- Save and reuse source and target rule tags.
- Preview every planned move before changing the file system.
- Skip conflicts or append a number without overwriting existing files.
- Verify cross-volume copies with SHA-256 before deleting the source.
- Cancel runs safely, retain run history, and detect files changed after preview.
- Import and export UTF-8 JSON settings.

## Run and build

Requirements: Node.js, pnpm, and Rust stable. Windows also requires Microsoft C++ Build Tools and WebView2. macOS requires Xcode Command Line Tools.

```powershell
pnpm install
pnpm desktop:dev
pnpm portable:build
```

The Windows portable build is created in `artifacts/portable/MoveMgr-win-x64/`. Settings and run history are stored beside the executable in `MoveMgrData`, so copy both `MoveMgr.exe` and `MoveMgrData` when moving the app to another computer.

## Verify

```powershell
pnpm check
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm version:check
```

## Documentation

- [User manual](docs/MANUAL.md)
- [Product and technical design](docs/DESIGN.md)
- [Data and command contracts](docs/CONTRACTS.md)
- [Implementation handoff](docs/IMPLEMENTATION_HANDOFF.md)
- [Settings example](docs/examples/settings-export.json)

If MoveMgr makes your file chores a little less annoying, please [give the project a GitHub star](https://github.com/parriernav/MoveMGR). It would make this tiny organizer very happy ★
