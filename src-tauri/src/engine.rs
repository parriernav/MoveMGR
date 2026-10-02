use crate::{models::*, rules};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};

fn modified_nanos(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn hidden(entry: &DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

fn display_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    if let Some(ordinary) = raw.strip_prefix(r"\\?\") {
        let bytes = ordinary.as_bytes();
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\'
        {
            return ordinary.to_string();
        }
    }
    raw.into_owned()
}

fn normalized_path_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) || cfg!(target_os = "macos") {
        value.to_lowercase()
    } else {
        value
    }
}

fn canonical_existing(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("폴더를 열 수 없습니다 ({}): {error}", path.display()))
}

fn roots_overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

fn reason(code: &str) -> String {
    match code {
        "KEY_TOO_SHORT" => "이름이 지정한 글자 수보다 짧습니다.",
        "DELIMITER_MISSING" => "이름에 구분자가 없습니다.",
        "EMPTY_KEY" => "분류값이 비어 있습니다.",
        "NO_TARGET_MATCH" => "일치하는 하위 폴더가 없습니다.",
        "AMBIGUOUS_TARGET" => "일치하는 하위 폴더가 여러 개입니다.",
        "INVALID_TARGET_NAME" => "분류값을 폴더 이름으로 사용할 수 없습니다.",
        "TARGET_EXISTS" => "같은 이름의 파일이 있습니다.",
        "TARGET_FOLDER_MISSING" => "목적지 하위 폴더가 없습니다.",
        _ => "파일을 처리할 수 없습니다.",
    }
    .into()
}

fn add_skipped(
    items: &mut Vec<InternalItem>,
    project: &Project,
    source: &Path,
    len: u64,
    modified: u128,
    key: Option<String>,
    code: &str,
) {
    items.push(InternalItem {
        public: PlannedItem {
            item_id: Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            source_path: display_path(source),
            extracted_key: key,
            proposed_target_path: None,
            decision: "skip".into(),
            reason_code: Some(code.into()),
            reason_text: Some(reason(code)),
            size_bytes: len.to_string(),
        },
        source: source.to_path_buf(),
        target: None,
        len,
        modified_nanos: modified,
        conflict: project.conflict,
    });
}

struct TargetSelection {
    directory: PathBuf,
    round_robin_group: Option<Vec<PathBuf>>,
}

impl From<PathBuf> for TargetSelection {
    fn from(directory: PathBuf) -> Self {
        Self {
            directory,
            round_robin_group: None,
        }
    }
}

fn matching_folder_index(
    folders: &[PathBuf],
    policy: MultipleMatchPolicy,
    round_robin_counts: &HashMap<Vec<PathBuf>, usize>,
) -> Result<usize, String> {
    match folders.len() {
        0 => Err("NO_TARGET_MATCH".into()),
        1 => Ok(0),
        _ => match policy {
            MultipleMatchPolicy::Skip => Err("AMBIGUOUS_TARGET".into()),
            MultipleMatchPolicy::First => Ok(0),
            MultipleMatchPolicy::RoundRobin => {
                Ok(round_robin_counts.get(folders).copied().unwrap_or(0) % folders.len())
            }
        },
    }
}

fn target_directory(
    project: &Project,
    key: Option<&str>,
    round_robin_counts: &HashMap<Vec<PathBuf>, usize>,
) -> Result<TargetSelection, String> {
    let root = PathBuf::from(&project.target.root);
    match &project.target.destination {
        Destination::Root => Ok(root.into()),
        Destination::FixedSubfolder {
            relative_path,
            create_if_missing,
        } => {
            let path = root.join(relative_path);
            if !path.exists() && !create_if_missing {
                Err("TARGET_FOLDER_MISSING".into())
            } else {
                Ok(path.into())
            }
        }
        Destination::KeySubfolder {
            parent_relative_path,
            create_if_missing,
        } => {
            let key = key.ok_or("EMPTY_KEY")?;
            if !rules::valid_key_folder(key) {
                return Err("INVALID_TARGET_NAME".into());
            }
            let path = root.join(parent_relative_path).join(key);
            if !path.exists() && !create_if_missing {
                Err("NO_TARGET_MATCH".into())
            } else {
                Ok(path.into())
            }
        }
        Destination::MatchSubfolder {
            search_base,
            folder_extractor,
            comparison,
            no_match,
            multiple_matches,
        } => {
            let key = key.ok_or("EMPTY_KEY")?;
            let base = root.join(search_base);
            if !base.is_dir() {
                return Err("TARGET_FOLDER_MISSING".into());
            }
            let mut matches = vec![];
            for entry in fs::read_dir(&base).map_err(|_| "TARGET_FOLDER_MISSING")? {
                let entry = entry.map_err(|_| "TARGET_FOLDER_MISSING")?;
                if !entry
                    .file_type()
                    .map_err(|_| "TARGET_FOLDER_MISSING")?
                    .is_dir()
                {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Ok(folder_key) = rules::extract(&name, folder_extractor, project.key.trim) {
                    if rules::folder_matches(
                        key,
                        &folder_key,
                        comparison,
                        project.comparison_options.ignore_case,
                    ) {
                        matches.push(entry.path());
                    }
                }
            }
            matches.sort_by_cached_key(|path| (normalized_path_key(path), path.clone()));
            if matches.is_empty() && matches!(no_match, NoMatch::CreateKeyFolder) {
                return if rules::valid_key_folder(key) {
                    Ok(base.join(key).into())
                } else {
                    Err("INVALID_TARGET_NAME".into())
                };
            }
            let index = matching_folder_index(&matches, *multiple_matches, round_robin_counts)?;
            let directory = matches[index].clone();
            let round_robin_group = (matches.len() > 1
                && matches!(multiple_matches, MultipleMatchPolicy::RoundRobin))
            .then_some(matches);
            Ok(TargetSelection {
                directory,
                round_robin_group,
            })
        }
    }
}

fn unique_target(
    mut target: PathBuf,
    conflict: Conflict,
    reserved: &mut HashSet<String>,
) -> Result<PathBuf, String> {
    let key = normalized_path_key(&target);
    if !target.exists() && !reserved.contains(&key) {
        reserved.insert(key);
        return Ok(target);
    }
    if matches!(conflict, Conflict::Skip) {
        return Err("TARGET_EXISTS".into());
    }
    let parent = target.parent().unwrap_or(Path::new("")).to_path_buf();
    let name = target
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("INVALID_TARGET_NAME")?;
    let (stem, extension) = rules::split_file_name(name);
    for number in 1..=9999 {
        let candidate_name = match extension {
            Some(ext) => format!("{stem} ({number}).{ext}"),
            None => format!("{stem} ({number})"),
        };
        let candidate = parent.join(candidate_name);
        let key = normalized_path_key(&candidate);
        if !candidate.exists() && !reserved.contains(&key) {
            reserved.insert(key);
            target = candidate;
            return Ok(target);
        }
    }
    Err("TARGET_EXISTS".into())
}

pub fn create_plan(state: &LocalState, project_ids: &[String]) -> Result<InternalPlan, String> {
    let selected: Vec<&Project> = project_ids
        .iter()
        .map(|id| {
            state
                .projects
                .iter()
                .find(|project| &project.id == id)
                .ok_or_else(|| format!("프로젝트를 찾을 수 없습니다: {id}"))
        })
        .collect::<Result<_, _>>()?;
    if selected.is_empty() {
        return Err("실행할 프로젝트가 없습니다.".into());
    }
    for project in &selected {
        rules::validate_project(project)?;
        if project.source.root.trim().is_empty() || project.target.root.trim().is_empty() {
            return Err(format!(
                "‘{}’의 소스와 타겟 폴더를 지정해주세요.",
                project.name
            ));
        }
    }

    let roots: Vec<(&Project, PathBuf, PathBuf)> = selected
        .iter()
        .map(|project| {
            let source = canonical_existing(Path::new(&project.source.root))?;
            let target = canonical_existing(Path::new(&project.target.root))?;
            if roots_overlap(&source, &target) {
                return Err(format!("‘{}’의 소스와 타겟 범위가 겹칩니다.", project.name));
            }
            Ok((*project, source, target))
        })
        .collect::<Result<_, String>>()?;

    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            if roots_overlap(&roots[i].1, &roots[j].1) {
                return Err(format!(
                    "‘{}’와 ‘{}’의 소스 범위가 겹칩니다.",
                    roots[i].0.name, roots[j].0.name
                ));
            }
            if roots_overlap(&roots[i].2, &roots[j].1) || roots_overlap(&roots[j].2, &roots[i].1) {
                return Err("한 프로젝트의 타겟이 다른 프로젝트의 소스와 겹칩니다.".into());
            }
        }
    }

    let mut reserved = HashSet::new();
    let mut items = vec![];
    for (project, source_root, _) in roots {
        // Each candidate-folder group rotates independently within this project's plan.
        let mut round_robin_counts = HashMap::new();
        let mut group_destinations: HashMap<(PathBuf, String), PathBuf> = HashMap::new();
        let walker = if project.source.recursive {
            WalkDir::new(&source_root)
        } else {
            WalkDir::new(&source_root).max_depth(1)
        };
        let mut paths: Vec<PathBuf> = walker
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                project.source.include_hidden || entry.depth() == 0 || !hidden(entry)
            })
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.depth() > 0 && entry.file_type().is_file() && !entry.file_type().is_symlink()
            })
            .map(|entry| entry.into_path())
            .collect();
        paths.sort_by_key(|path| {
            normalized_path_key(path.strip_prefix(&source_root).unwrap_or(path))
        });
        for source in paths {
            let file_name = match source.file_name().and_then(|value| value.to_str()) {
                Some(value) => value,
                None => continue,
            };
            let (stem, extension) = rules::split_file_name(file_name);
            if !rules::file_matches(project, stem, extension) {
                continue;
            }
            let metadata = match fs::metadata(&source) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let len = metadata.len();
            let modified = modified_nanos(&metadata);
            let source_group =
                matches!(project.source.move_unit, MoveUnit::SameNameGroup).then(|| {
                    (
                        source.parent().unwrap_or(&source_root).to_path_buf(),
                        rules::normalize(stem, project.comparison_options.ignore_case),
                    )
                });
            let needs_key = !matches!(
                project.target.destination,
                Destination::Root | Destination::FixedSubfolder { .. }
            );
            let key = if needs_key {
                match rules::extract(stem, &project.key.extractor, project.key.trim) {
                    Ok(value) => Some(value),
                    Err(code) => {
                        add_skipped(&mut items, project, &source, len, modified, None, code);
                        continue;
                    }
                }
            } else {
                None
            };
            let selection = if let Some(directory) = source_group
                .as_ref()
                .and_then(|group| group_destinations.get(group))
            {
                TargetSelection::from(directory.clone())
            } else {
                match target_directory(project, key.as_deref(), &round_robin_counts) {
                    Ok(value) => value,
                    Err(code) => {
                        add_skipped(&mut items, project, &source, len, modified, key, &code);
                        continue;
                    }
                }
            };
            let target = match unique_target(
                selection.directory.join(file_name),
                project.conflict,
                &mut reserved,
            ) {
                Ok(value) => value,
                Err(code) => {
                    add_skipped(&mut items, project, &source, len, modified, key, &code);
                    continue;
                }
            };
            // Conflicts that skip a file must not consume a folder's turn.
            if let Some(group) = selection.round_robin_group {
                *round_robin_counts.entry(group).or_insert(0) += 1;
            }
            // The first movable member anchors the bundle; later members reuse its turn.
            if let Some(group) = source_group {
                group_destinations
                    .entry(group)
                    .or_insert(selection.directory);
            }
            let public = PlannedItem {
                item_id: Uuid::new_v4().to_string(),
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                source_path: display_path(&source),
                extracted_key: key,
                proposed_target_path: Some(display_path(&target)),
                decision: "move".into(),
                reason_code: None,
                reason_text: None,
                size_bytes: len.to_string(),
            };
            items.push(InternalItem {
                public,
                source,
                target: Some(target),
                len,
                modified_nanos: modified,
                conflict: project.conflict,
            });
        }
    }
    let public_items: Vec<PlannedItem> = items.iter().map(|item| item.public.clone()).collect();
    let plan = Plan {
        plan_id: Uuid::new_v4().to_string(),
        created_at: Utc::now().to_rfc3339(),
        revision: state.revision,
        movable: public_items
            .iter()
            .filter(|item| item.decision == "move")
            .count(),
        skipped: public_items
            .iter()
            .filter(|item| item.decision == "skip")
            .count(),
        blocked: public_items
            .iter()
            .filter(|item| item.decision == "blocked")
            .count(),
        items: public_items,
    };
    Ok(InternalPlan {
        public: plan,
        items,
    })
}

fn hash_file(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_vec())
}

fn runtime_target(target: &Path, conflict: Conflict) -> Result<PathBuf, String> {
    let mut reserved = HashSet::new();
    unique_target(target.to_path_buf(), conflict, &mut reserved)
}

enum MoveOutcome {
    Moved(PathBuf),
    SourceRetained(PathBuf, String),
}

fn move_verified(
    item: &InternalItem,
    run_id: &str,
    cancel: &AtomicBool,
    try_source_hard_link: bool,
) -> Result<MoveOutcome, String> {
    let metadata =
        fs::metadata(&item.source).map_err(|error| format!("원본을 읽을 수 없습니다: {error}"))?;
    if metadata.len() != item.len || modified_nanos(&metadata) != item.modified_nanos {
        return Err("미리보기 이후 원본이 변경되었습니다.".into());
    }
    let planned_target = item.target.as_ref().ok_or("목적지가 없습니다.")?;
    fs::create_dir_all(planned_target.parent().ok_or("목적지 폴더가 없습니다.")?)
        .map_err(|error| format!("목적지 폴더 생성 실패: {error}"))?;
    let target = runtime_target(planned_target, item.conflict)?;
    if try_source_hard_link && fs::hard_link(&item.source, &target).is_ok() {
        return match fs::remove_file(&item.source) {
            Ok(()) => Ok(MoveOutcome::Moved(target)),
            Err(error) => Ok(MoveOutcome::SourceRetained(
                target,
                format!("복사는 완료됐지만 원본 삭제에 실패했습니다: {error}"),
            )),
        };
    }

    let parent = target.parent().ok_or("목적지 폴더가 없습니다.")?;
    let part = parent.join(format!(".movemgr-{run_id}-{}.part", item.public.item_id));
    let mut source_file =
        fs::File::open(&item.source).map_err(|error| format!("원본 열기 실패: {error}"))?;
    let mut part_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .map_err(|error| format!("임시 파일 생성 실패: {error}"))?;
    let mut source_hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let copy_result = (|| -> Result<(), String> {
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("사용자가 실행을 취소했습니다.".into());
            }
            let count = source_file
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            part_file
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            source_hash.update(&buffer[..count]);
        }
        part_file.sync_all().map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Err(error) = copy_result {
        let _ = fs::remove_file(&part);
        return Err(format!("복사 실패: {error}"));
    }
    let source_digest = source_hash.finalize().to_vec();
    let target_digest = hash_file(&part).map_err(|error| {
        let _ = fs::remove_file(&part);
        format!("복사 검증 실패: {error}")
    })?;
    if source_digest != target_digest {
        let _ = fs::remove_file(&part);
        return Err("복사한 파일의 SHA-256 검증에 실패했습니다.".into());
    }
    let after = fs::metadata(&item.source).map_err(|error| error.to_string())?;
    if after.len() != item.len || modified_nanos(&after) != item.modified_nanos {
        let _ = fs::remove_file(&part);
        return Err("복사 중 원본이 변경되었습니다.".into());
    }
    if let Err(error) = fs::hard_link(&part, &target) {
        let _ = fs::remove_file(&part);
        return Err(format!("최종 파일 확정 실패: {error}"));
    }
    let _ = fs::remove_file(&part);
    match fs::remove_file(&item.source) {
        Ok(()) => Ok(MoveOutcome::Moved(target)),
        Err(error) => Ok(MoveOutcome::SourceRetained(
            target,
            format!("복사는 완료됐지만 원본 삭제에 실패했습니다: {error}"),
        )),
    }
}

pub fn execute_plan(
    plan: InternalPlan,
    journal_dir: &Path,
    cancel: &AtomicBool,
) -> Result<RunResult, String> {
    let run_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    fs::create_dir_all(journal_dir.join(&run_id)).map_err(|error| error.to_string())?;
    let journal_path = journal_dir.join(&run_id).join("journal.jsonl");
    let mut journal = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(journal_path)
        .map_err(|error| format!("실행 기록 생성 실패: {error}"))?;
    let mut result = RunResult {
        run_id: run_id.clone(),
        started_at,
        finished_at: String::new(),
        moved: 0,
        skipped: 0,
        failed: 0,
        source_retained: 0,
        items: vec![],
    };
    for item in plan.items {
        if cancel.load(Ordering::Acquire) {
            result.skipped += 1;
            result.items.push(RunItem {
                planned: item.public,
                state: "cancelled".into(),
                final_target_path: None,
                message: Some("사용자가 실행을 취소했습니다.".into()),
            });
            continue;
        }
        if item.public.decision != "move" {
            result.skipped += 1;
            result.items.push(RunItem {
                planned: item.public,
                state: "skipped".into(),
                final_target_path: None,
                message: None,
            });
            continue;
        }
        writeln!(journal, "{}", serde_json::json!({"itemId": item.public.item_id, "state": "planned", "source": item.source, "target": item.target})).map_err(|error| format!("실행 기록 쓰기 실패: {error}"))?;
        journal
            .sync_all()
            .map_err(|error| format!("실행 기록 저장 실패: {error}"))?;
        match move_verified(&item, &run_id, cancel, true) {
            Ok(MoveOutcome::Moved(target)) => {
                result.moved += 1;
                result.items.push(RunItem {
                    planned: item.public.clone(),
                    state: "moved".into(),
                    final_target_path: Some(display_path(&target)),
                    message: None,
                });
                writeln!(journal, "{}", serde_json::json!({"itemId": item.public.item_id, "state": "moved", "target": target})).map_err(|error| error.to_string())?;
            }
            Ok(MoveOutcome::SourceRetained(target, message)) => {
                result.source_retained += 1;
                result.items.push(RunItem {
                    planned: item.public.clone(),
                    state: "sourceRetained".into(),
                    final_target_path: Some(display_path(&target)),
                    message: Some(message.clone()),
                });
                writeln!(journal, "{}", serde_json::json!({"itemId": item.public.item_id, "state": "sourceRetained", "target": target, "message": message})).map_err(|error| error.to_string())?;
            }
            Err(message) => {
                result.failed += 1;
                result.items.push(RunItem {
                    planned: item.public.clone(),
                    state: "failed".into(),
                    final_target_path: None,
                    message: Some(message.clone()),
                });
                writeln!(journal, "{}", serde_json::json!({"itemId": item.public.item_id, "state": "failed", "message": message})).map_err(|error| error.to_string())?;
            }
        }
        journal
            .sync_all()
            .map_err(|error| format!("실행 기록 저장 실패: {error}"))?;
    }
    result.finished_at = Utc::now().to_rfc3339();
    Ok(result)
}

pub fn evaluate_example(
    project: &Project,
    sample_name: &str,
    sample_folders: &[String],
) -> Result<serde_json::Value, String> {
    rules::validate_project(project)?;
    let (stem, extension) = rules::split_file_name(sample_name);
    if !rules::file_matches(project, stem, extension) {
        return Ok(
            serde_json::json!({"key": null, "targetFolder": null, "reason": "파일 조건에 맞지 않습니다."}),
        );
    }
    let needs_key = !matches!(
        project.target.destination,
        Destination::Root | Destination::FixedSubfolder { .. }
    );
    let key = if needs_key {
        Some(rules::extract(stem, &project.key.extractor, project.key.trim).map_err(reason)?)
    } else {
        None
    };
    let folder = match &project.target.destination {
        Destination::Root => Some(project.target.root.clone()),
        Destination::FixedSubfolder { relative_path, .. } => Some(
            Path::new(&project.target.root)
                .join(relative_path)
                .to_string_lossy()
                .into_owned(),
        ),
        Destination::KeySubfolder {
            parent_relative_path,
            ..
        } => Some(
            Path::new(&project.target.root)
                .join(parent_relative_path)
                .join(key.as_deref().unwrap_or(""))
                .to_string_lossy()
                .into_owned(),
        ),
        Destination::MatchSubfolder {
            folder_extractor,
            comparison,
            multiple_matches,
            ..
        } => {
            let mut matching: Vec<PathBuf> = sample_folders
                .iter()
                .filter(|folder| {
                    rules::extract(folder, folder_extractor, project.key.trim)
                        .map(|folder_key| {
                            rules::folder_matches(
                                key.as_deref().unwrap_or(""),
                                &folder_key,
                                comparison,
                                project.comparison_options.ignore_case,
                            )
                        })
                        .unwrap_or(false)
                })
                .map(PathBuf::from)
                .collect();
            matching.sort_by_cached_key(|path| (normalized_path_key(path), path.clone()));
            // A single-file example begins at the first turn, like a fresh plan.
            matching_folder_index(&matching, *multiple_matches, &HashMap::new())
                .ok()
                .map(|index| matching[index].to_string_lossy().into_owned())
        }
    };
    Ok(serde_json::json!({"key": key, "targetFolder": folder, "reason": null}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    #[test]
    fn display_paths_hide_windows_verbatim_prefixes() {
        assert_eq!(
            display_path(Path::new(r"\\?\E:\ai_pixel\image.png")),
            r"E:\ai_pixel\image.png"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share\image.png")),
            r"\\server\share\image.png"
        );
        assert_eq!(
            display_path(Path::new(r"E:\ordinary\image.png")),
            r"E:\ordinary\image.png"
        );
    }

    fn project(
        source: &Path,
        target: &Path,
        destination: Destination,
        conflict: Conflict,
    ) -> Project {
        Project {
            id: Uuid::new_v4().to_string(),
            name: "테스트 프로젝트".into(),
            checked: true,
            source: Source {
                root: source.to_string_lossy().into_owned(),
                recursive: false,
                include_hidden: false,
                move_unit: MoveUnit::File,
                extensions: ExtensionFilter::Only {
                    values: vec!["pdf".into()],
                    include_extensionless: false,
                },
                name_filters: vec![],
            },
            key: KeyRule {
                extractor: Extractor::BeforeDelimiter {
                    delimiter: "_".into(),
                    missing: MissingDelimiter::Skip,
                },
                trim: true,
            },
            comparison_options: ComparisonOptions {
                ignore_case: true,
                normalization: "NFC".into(),
            },
            target: Target {
                root: target.to_string_lossy().into_owned(),
                destination,
            },
            conflict,
        }
    }

    fn state(project: Project) -> LocalState {
        LocalState {
            schema_version: 1,
            revision: 7,
            preferences: Preferences {
                theme: "system".into(),
                language: "ko".into(),
                preview_before_run: true,
            },
            projects: vec![project],
            rule_tags: vec![],
        }
    }

    fn matching_destination(policy: MultipleMatchPolicy) -> Destination {
        Destination::MatchSubfolder {
            search_base: String::new(),
            folder_extractor: Extractor::BeforeDelimiter {
                delimiter: "_".into(),
                missing: MissingDelimiter::UseWhole,
            },
            comparison: Comparison::Equals,
            no_match: NoMatch::Skip,
            multiple_matches: policy,
        }
    }

    fn media_project(
        source: &Path,
        target: &Path,
        move_unit: MoveUnit,
        conflict: Conflict,
    ) -> Project {
        let mut project = project(
            source,
            target,
            matching_destination(MultipleMatchPolicy::RoundRobin),
            conflict,
        );
        project.source.extensions = ExtensionFilter::All;
        project.source.move_unit = move_unit;
        project
    }

    #[test]
    fn same_name_media_groups_share_one_round_robin_turn_and_execute_together() {
        for (unit, expected_folders) in [
            (
                MoveUnit::File,
                [
                    "client_a", "client_b", "client_c", "client_a", "client_b", "client_c",
                ],
            ),
            (
                MoveUnit::SameNameGroup,
                [
                    "client_a", "client_a", "client_a", "client_b", "client_b", "client_b",
                ],
            ),
        ] {
            let temp = TempDir::new().unwrap();
            let source = temp.path().join("source");
            let target = temp.path().join("target");
            fs::create_dir_all(&source).unwrap();
            for folder in ["client_c", "client_b", "client_a"] {
                fs::create_dir_all(target.join(folder)).unwrap();
            }
            for stem in ["client_1", "client_2"] {
                for extension in ["mp4", "txt", "png"] {
                    let name = format!("{stem}.{extension}");
                    fs::write(source.join(&name), name.as_bytes()).unwrap();
                }
            }
            let project = media_project(&source, &target, unit, Conflict::Skip);
            let id = project.id.clone();
            let state = state(project);
            let plan = create_plan(&state, &[id.clone()]).unwrap();
            assert_eq!(plan.public.movable, 6);
            let planned: Vec<PathBuf> = plan
                .items
                .iter()
                .map(|item| item.target.clone().unwrap())
                .collect();
            for (path, folder) in planned.iter().zip(expected_folders) {
                assert_eq!(path.parent().unwrap(), target.join(folder));
            }
            let repeated = create_plan(&state, &[id]).unwrap();
            assert_eq!(
                repeated
                    .items
                    .iter()
                    .map(|item| item.target.clone().unwrap())
                    .collect::<Vec<_>>(),
                planned
            );
            assert_eq!(fs::read_dir(&source).unwrap().count(), 6);
            for folder in ["client_a", "client_b", "client_c"] {
                assert_eq!(fs::read_dir(target.join(folder)).unwrap().count(), 0);
            }
            let result =
                execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
            assert_eq!(result.moved, 6);
            assert_eq!(result.failed, 0);
            assert_eq!(fs::read_dir(&source).unwrap().count(), 0);
            for path in planned {
                assert_eq!(
                    fs::read(&path).unwrap(),
                    path.file_name().unwrap().to_str().unwrap().as_bytes()
                );
            }
        }
    }

    #[test]
    fn bundles_use_the_full_name_and_reuse_destinations_for_nonadjacent_members() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        for folder in ["client_a", "client_b", "client_c"] {
            fs::create_dir_all(target.join(folder)).unwrap();
        }
        for name in [
            "client_clip",
            "client_clip.mp4",
            "client_clip.part.mp4",
            "client_clip.png",
            "client_clip.txt",
            "client_next.mp4",
        ] {
            fs::write(source.join(name), b"new").unwrap();
        }
        let project = media_project(&source, &target, MoveUnit::SameNameGroup, Conflict::Skip);
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();
        assert_eq!(plan.public.movable, 6);
        for item in plan.items {
            let name = item.source.file_name().unwrap().to_str().unwrap();
            let expected = match name {
                "client_clip.part.mp4" => "client_b",
                "client_next.mp4" => "client_c",
                _ => "client_a",
            };
            assert_eq!(
                item.target.unwrap().parent().unwrap(),
                target.join(expected)
            );
        }
    }

    #[test]
    fn identical_names_in_different_source_subfolders_are_separate_bundles() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        for folder in ["client_a", "client_b"] {
            fs::create_dir_all(target.join(folder)).unwrap();
        }
        for folder in ["one", "two"] {
            fs::create_dir_all(source.join(folder)).unwrap();
            for name in ["client_clip.mp4", "client_clip.txt"] {
                fs::write(source.join(folder).join(name), b"new").unwrap();
            }
        }
        let mut project = media_project(&source, &target, MoveUnit::SameNameGroup, Conflict::Skip);
        project.source.recursive = true;
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();
        assert_eq!(plan.public.movable, 4);
        for (item, folder) in plan
            .items
            .iter()
            .zip(["client_a", "client_a", "client_b", "client_b"])
        {
            assert_eq!(
                item.target.as_ref().unwrap().parent().unwrap(),
                target.join(folder)
            );
        }
    }

    #[test]
    fn grouping_respects_source_filters_and_does_not_move_excluded_sidecars() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        for folder in ["client_a", "client_b"] {
            fs::create_dir_all(target.join(folder)).unwrap();
        }
        for name in [
            "client_1.mp4",
            "client_1.png",
            "client_1.txt",
            "client_2.mp4",
            "client_other.mp4",
        ] {
            fs::write(source.join(name), b"new").unwrap();
        }
        let mut project = media_project(&source, &target, MoveUnit::SameNameGroup, Conflict::Skip);
        project.source.extensions = ExtensionFilter::Only {
            values: vec!["mp4".into(), "png".into()],
            include_extensionless: false,
        };
        project.source.name_filters = vec![
            NameFilter {
                op: NameFilterOp::StartsWith,
                value: "client_".into(),
            },
            NameFilter {
                op: NameFilterOp::Contains,
                value: "1".into(),
            },
        ];
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();
        assert_eq!(plan.public.movable, 2);
        assert!(
            plan.items
                .iter()
                .all(|item| item.target.as_ref().unwrap().parent().unwrap()
                    == target.join("client_a"))
        );
        let result =
            execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.moved, 2);
        assert!(source.join("client_1.txt").exists());
        assert!(source.join("client_2.mp4").exists());
        assert!(source.join("client_other.mp4").exists());
    }

    #[test]
    fn bundle_conflicts_do_not_consume_extra_turns_or_change_the_groups_folder() {
        for conflict in [Conflict::Skip, Conflict::RenameWithNumber] {
            let temp = TempDir::new().unwrap();
            let source = temp.path().join("source");
            let target = temp.path().join("target");
            fs::create_dir_all(&source).unwrap();
            for folder in ["client_a", "client_b", "client_c"] {
                fs::create_dir_all(target.join(folder)).unwrap();
            }
            for name in [
                "client_1.mp4",
                "client_1.png",
                "client_1.txt",
                "client_2.mp4",
                "client_3.mp4",
                "client_3.txt",
                "client_4.mp4",
            ] {
                fs::write(source.join(name), b"new").unwrap();
            }
            fs::write(target.join("client_a/client_1.mp4"), b"old").unwrap();
            fs::write(target.join("client_a/client_1.txt"), b"old").unwrap();
            fs::write(target.join("client_c/client_3.mp4"), b"old").unwrap();
            fs::write(target.join("client_c/client_3.txt"), b"old").unwrap();
            let project = media_project(&source, &target, MoveUnit::SameNameGroup, conflict);
            let id = project.id.clone();
            let plan = create_plan(&state(project), &[id]).unwrap();
            let expected_last = if matches!(conflict, Conflict::Skip) {
                "client_c"
            } else {
                "client_a"
            };
            for item in &plan.items {
                if let Some(path) = &item.target {
                    let stem =
                        rules::split_file_name(item.source.file_name().unwrap().to_str().unwrap())
                            .0;
                    let expected = match stem {
                        "client_1" => "client_a",
                        "client_2" => "client_b",
                        "client_3" => "client_c",
                        _ => expected_last,
                    };
                    assert_eq!(path.parent().unwrap(), target.join(expected));
                }
            }
            assert_eq!(
                plan.public.skipped,
                if matches!(conflict, Conflict::Skip) {
                    4
                } else {
                    0
                }
            );
            let result =
                execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
            assert_eq!(result.failed, 0);
            assert_eq!(
                fs::read(target.join("client_a/client_1.mp4")).unwrap(),
                b"old"
            );
        }
    }

    #[test]
    fn bundle_names_follow_unicode_normalization_and_ignore_case() {
        for (names, ignore_case, same_bundle) in [
            (["client_Clip.mp4", "client_clip.txt"], true, true),
            (["client_Clip.mp4", "client_clip.txt"], false, false),
            (
                ["client_\u{1100}\u{1161}.mp4", "client_가.txt"],
                false,
                true,
            ),
        ] {
            let temp = TempDir::new().unwrap();
            let source = temp.path().join("source");
            let target = temp.path().join("target");
            fs::create_dir_all(&source).unwrap();
            for folder in ["client_a", "client_b"] {
                fs::create_dir_all(target.join(folder)).unwrap();
            }
            for name in names {
                fs::write(source.join(name), b"new").unwrap();
            }
            let mut project =
                media_project(&source, &target, MoveUnit::SameNameGroup, Conflict::Skip);
            project.comparison_options.ignore_case = ignore_case;
            let id = project.id.clone();
            let plan = create_plan(&state(project), &[id]).unwrap();
            assert_eq!(plan.public.movable, 2);
            let first = plan.items[0].target.as_ref().unwrap().parent().unwrap();
            let second = plan.items[1].target.as_ref().unwrap().parent().unwrap();
            assert_eq!(first == second, same_bundle);
        }
    }

    #[test]
    fn multiple_match_policies_plan_and_execute_in_folder_name_order() {
        for (policy, expected) in [
            (MultipleMatchPolicy::Skip, vec![]),
            (MultipleMatchPolicy::First, vec!["client_a"; 5]),
            (
                MultipleMatchPolicy::RoundRobin,
                vec!["client_a", "client_b", "client_c", "client_a", "client_b"],
            ),
        ] {
            let temp = TempDir::new().unwrap();
            let source = temp.path().join("source");
            let target = temp.path().join("target");
            fs::create_dir_all(&source).unwrap();
            for folder in ["client_c", "client_b", "client_a"] {
                fs::create_dir_all(target.join(folder)).unwrap();
            }
            for index in 1..=5 {
                fs::write(source.join(format!("client_{index}.pdf")), b"new").unwrap();
            }
            let project = project(
                &source,
                &target,
                matching_destination(policy),
                Conflict::Skip,
            );
            let id = project.id.clone();
            let state = state(project);
            let plan = create_plan(&state, &[id.clone()]).unwrap();
            let planned: Vec<PathBuf> = plan
                .items
                .iter()
                .filter_map(|item| item.target.clone())
                .collect();
            let expected_paths: Vec<PathBuf> = expected
                .iter()
                .enumerate()
                .map(|(index, folder)| {
                    target
                        .join(folder)
                        .join(format!("client_{}.pdf", index + 1))
                })
                .collect();
            assert_eq!(planned, expected_paths);
            assert_eq!(plan.public.skipped, 5 - expected.len());
            if expected.is_empty() {
                assert!(plan
                    .public
                    .items
                    .iter()
                    .all(|item| item.reason_code.as_deref() == Some("AMBIGUOUS_TARGET")));
            }
            // Repeated previews restart rotation without mutating source or target contents.
            let repeated = create_plan(&state, &[id]).unwrap();
            assert_eq!(
                repeated
                    .items
                    .iter()
                    .filter_map(|item| item.target.clone())
                    .collect::<Vec<_>>(),
                planned
            );
            assert_eq!(fs::read_dir(&source).unwrap().count(), 5);
            let result =
                execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
            assert_eq!(result.moved, expected.len());
            assert_eq!(result.failed, 0);
            for path in planned {
                assert_eq!(fs::read(path).unwrap(), b"new");
            }
        }
    }

    #[test]
    fn round_robin_groups_equivalent_keys_and_keeps_projects_independent() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target");
        for folder in ["alpha_a", "alpha_b", "beta_a", "beta_b"] {
            fs::create_dir_all(target.join(folder)).unwrap();
        }
        let mut projects = vec![];
        for index in 1..=2 {
            let source = temp.path().join(format!("source{index}"));
            for (subfolder, name) in [
                ("01", "ALPHA1_doc.pdf"),
                ("02", "BETA1_doc.pdf"),
                ("03", "alpha2_doc.pdf"),
                ("04", "beta2_doc.pdf"),
            ] {
                fs::create_dir_all(source.join(subfolder)).unwrap();
                fs::write(source.join(subfolder).join(name), b"new").unwrap();
            }
            let mut destination = matching_destination(MultipleMatchPolicy::RoundRobin);
            if let Destination::MatchSubfolder { comparison, .. } = &mut destination {
                *comparison = Comparison::PrefixEqual { count: 4 };
            }
            let mut project = project(&source, &target, destination, Conflict::RenameWithNumber);
            project.source.recursive = true;
            projects.push(project);
        }
        let ids: Vec<String> = projects.iter().map(|project| project.id.clone()).collect();
        let mut state = state(projects[0].clone());
        state.projects = projects;
        let plan = create_plan(&state, &ids).unwrap();
        assert_eq!(plan.public.movable, 8);
        let folders: Vec<&str> = plan
            .items
            .iter()
            .map(|item| {
                item.target
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            folders,
            ["alpha_a", "beta_a", "alpha_b", "beta_b", "alpha_a", "beta_a", "alpha_b", "beta_b"]
        );
    }

    #[test]
    fn skipped_conflicts_do_not_consume_round_robin_turns() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        for folder in ["client_a", "client_b"] {
            fs::create_dir_all(target.join(folder)).unwrap();
        }
        for index in 1..=3 {
            fs::write(source.join(format!("client_{index}.pdf")), b"new").unwrap();
        }
        fs::write(target.join("client_a/client_1.pdf"), b"existing").unwrap();
        let project = project(
            &source,
            &target,
            matching_destination(MultipleMatchPolicy::RoundRobin),
            Conflict::Skip,
        );
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();
        assert_eq!(
            plan.public.items[0].reason_code.as_deref(),
            Some("TARGET_EXISTS")
        );
        assert_eq!(
            plan.items[1].target.as_ref().unwrap(),
            &target.join("client_a/client_2.pdf")
        );
        assert_eq!(
            plan.items[2].target.as_ref().unwrap(),
            &target.join("client_b/client_3.pdf")
        );
        let result =
            execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.moved, 2);
        assert_eq!(
            fs::read(target.join("client_a/client_1.pdf")).unwrap(),
            b"existing"
        );
        assert!(source.join("client_1.pdf").exists());
    }

    #[test]
    fn match_policies_preserve_single_folder_and_no_match_behavior() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(target.join("client_a")).unwrap();
        for policy in [
            MultipleMatchPolicy::Skip,
            MultipleMatchPolicy::First,
            MultipleMatchPolicy::RoundRobin,
        ] {
            let mut project = project(
                &source,
                &target,
                matching_destination(policy),
                Conflict::Skip,
            );
            assert_eq!(
                target_directory(&project, Some("client"), &HashMap::new())
                    .unwrap()
                    .directory,
                target.join("client_a")
            );
            assert_eq!(
                target_directory(&project, Some("missing"), &HashMap::new())
                    .err()
                    .unwrap(),
                "NO_TARGET_MATCH"
            );
            if let Destination::MatchSubfolder { no_match, .. } = &mut project.target.destination {
                *no_match = NoMatch::CreateKeyFolder;
            }
            assert_eq!(
                target_directory(&project, Some("missing"), &HashMap::new())
                    .unwrap()
                    .directory,
                target.join("missing")
            );
            assert!(!target.join("missing").exists());
        }
    }

    #[test]
    fn examples_honor_multiple_match_policies() {
        for (policy, expected) in [
            (MultipleMatchPolicy::Skip, serde_json::Value::Null),
            (MultipleMatchPolicy::First, serde_json::json!("client_a")),
            (
                MultipleMatchPolicy::RoundRobin,
                serde_json::json!("client_a"),
            ),
        ] {
            let project = project(
                Path::new("source"),
                Path::new("target"),
                matching_destination(policy),
                Conflict::Skip,
            );
            let result = evaluate_example(
                &project,
                "client_doc.pdf",
                &["client_b".into(), "client_a".into()],
            )
            .unwrap();
            assert_eq!(result["targetFolder"], expected);
        }
    }

    #[test]
    fn preview_does_not_modify_disk_and_execution_moves_file() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("거래처A_견적서.pdf"), b"move me").unwrap();
        let project = project(
            &source,
            &target,
            Destination::KeySubfolder {
                parent_relative_path: String::new(),
                create_if_missing: true,
            },
            Conflict::Skip,
        );
        let id = project.id.clone();

        let plan = create_plan(&state(project), &[id]).unwrap();
        assert_eq!(plan.public.movable, 1);
        assert!(
            !target.join("거래처A").exists(),
            "미리보기는 폴더를 만들면 안 됩니다"
        );

        let result =
            execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.moved, 1);
        assert!(!source.join("거래처A_견적서.pdf").exists());
        assert_eq!(
            fs::read(target.join("거래처A").join("거래처A_견적서.pdf")).unwrap(),
            b"move me"
        );
    }

    #[test]
    fn collision_uses_number_without_overwriting() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("보고서.pdf"), b"new").unwrap();
        fs::write(target.join("보고서.pdf"), b"old").unwrap();
        let project = project(
            &source,
            &target,
            Destination::Root,
            Conflict::RenameWithNumber,
        );
        let id = project.id.clone();

        let plan = create_plan(&state(project), &[id]).unwrap();
        assert!(plan.public.items[0]
            .proposed_target_path
            .as_deref()
            .unwrap()
            .ends_with("보고서 (1).pdf"));
        let result =
            execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.moved, 1);
        assert_eq!(fs::read(target.join("보고서.pdf")).unwrap(), b"old");
        assert_eq!(fs::read(target.join("보고서 (1).pdf")).unwrap(), b"new");
    }

    #[test]
    fn copy_fallback_verifies_large_file_without_exhausting_worker_stack() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        let contents = vec![0x5a; 2 * 1024 * 1024 + 17];
        let source_file = source.join("large.pdf");
        fs::write(&source_file, &contents).unwrap();
        let project = project(&source, &target, Destination::Root, Conflict::Skip);
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();

        let outcome =
            move_verified(&plan.items[0], "test-run", &AtomicBool::new(false), false).unwrap();
        assert!(matches!(outcome, MoveOutcome::Moved(_)));
        assert!(!source_file.exists());
        assert_eq!(fs::read(target.join("large.pdf")).unwrap(), contents);
    }

    #[test]
    fn source_change_after_preview_is_blocked() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        let source_file = source.join("보고서.pdf");
        fs::write(&source_file, b"before").unwrap();
        let project = project(&source, &target, Destination::Root, Conflict::Skip);
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();
        fs::write(&source_file, b"changed and longer").unwrap();

        let result =
            execute_plan(plan, &temp.path().join("runs"), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.failed, 1);
        assert!(source_file.exists());
        assert!(!target.join("보고서.pdf").exists());
    }

    #[test]
    fn cancellation_keeps_unstarted_sources() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("하나.pdf"), b"one").unwrap();
        fs::write(source.join("둘.pdf"), b"two").unwrap();
        let project = project(&source, &target, Destination::Root, Conflict::Skip);
        let id = project.id.clone();
        let plan = create_plan(&state(project), &[id]).unwrap();
        let cancel = AtomicBool::new(true);

        let result = execute_plan(plan, &temp.path().join("runs"), &cancel).unwrap();
        assert_eq!(result.skipped, 2);
        assert!(source.join("하나.pdf").exists());
        assert!(source.join("둘.pdf").exists());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    }
}
