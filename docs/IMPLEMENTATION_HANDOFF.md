# Implementation Handoff

## Scope

MoveMgr is a Tauri 2 desktop application with a Svelte 5 frontend and a Rust file-operation engine. It supports Windows and macOS, while the portable distribution workflow targets Windows.

## Implementation order

1. Keep shared TypeScript and Rust models compatible with the JSON schema.
2. Validate projects and reusable rule tags before persistence.
3. Keep planning read-only and execution bound to an immutable plan ID.
4. Preserve no-overwrite and cross-volume verification guarantees.
5. Keep every visible frontend string in the localization table, except the English-only manual.
6. Synchronize versions through `version.json` before packaging.

## Required acceptance checks

- A new installation starts in English.
- English, Korean, Japanese, and Simplified Chinese switch immediately and persist.
- The manual contains English only.
- Preview does not modify the file system.
- Existing targets are never overwritten.
- A changed source after preview is blocked.
- Cross-volume copies are hash-verified before source deletion.
- Cancellation leaves unstarted sources untouched.
- Settings import rejects invalid schemas and duplicate IDs.
- Frontend checks, unit tests, production build, Rust tests, and version checks pass.

## Commands

```powershell
pnpm check
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm version:check
```
