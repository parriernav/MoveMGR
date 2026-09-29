mod engine;
mod models;
mod rules;
mod storage;

use models::*;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Manager, State};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

struct AppState {
    settings: Mutex<LocalState>,
    plans: Mutex<HashMap<String, InternalPlan>>,
    running: AtomicBool,
    cancel: Arc<AtomicBool>,
}

fn validate_rule_tags(tags: &[RuleTag]) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for tag in tags {
        if Uuid::parse_str(tag.id()).is_err() || !ids.insert(tag.id().to_string()) {
            return Err("태그 ID가 올바르지 않거나 중복되었습니다.".into());
        }
        let name = tag.name().trim();
        if name.is_empty() || name.graphemes(true).count() > 80 {
            return Err("태그 이름은 1~80글자로 입력해주세요.".into());
        }
        if !names.insert((tag.kind(), name.to_lowercase())) {
            return Err("같은 종류에 동일한 이름의 태그가 있습니다.".into());
        }
        rules::validate_rule_tag(tag)?;
    }
    Ok(())
}

#[tauri::command]
fn get_app_info(app: AppHandle) -> Result<AppInfo, String> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS.into(),
        config_dir: storage::config_dir(&app)?.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
fn load_state(state: State<AppState>) -> Result<LocalState, String> {
    state
        .settings
        .lock()
        .map_err(|_| "설정 잠금이 손상되었습니다.".to_string())
        .map(|value| value.clone())
}

#[tauri::command]
fn save_state(
    app: AppHandle,
    state: State<AppState>,
    mut next_state: LocalState,
    expected_revision: u64,
) -> Result<LocalState, String> {
    let mut current = state
        .settings
        .lock()
        .map_err(|_| "설정 잠금이 손상되었습니다.".to_string())?;
    if current.revision != expected_revision {
        return Err("설정이 다른 작업에서 변경되었습니다. 다시 불러온 뒤 시도해주세요.".into());
    }
    if next_state.schema_version != 1 {
        return Err("지원하지 않는 설정 버전입니다.".into());
    }
    if !matches!(
        next_state.preferences.language.as_str(),
        "ko" | "en" | "ja" | "zh"
    )
        || !matches!(
            next_state.preferences.theme.as_str(),
            "system" | "light" | "dark"
        )
    {
        return Err("올바르지 않은 앱 설정입니다.".into());
    }
    let mut ids = HashSet::new();
    for project in &next_state.projects {
        rules::validate_project(project)?;
        if !ids.insert(project.id.clone()) {
            return Err("중복된 프로젝트 ID가 있습니다.".into());
        }
    }
    validate_rule_tags(&next_state.rule_tags)?;
    next_state.revision = current.revision + 1;
    storage::write_state(&app, &next_state)?;
    *current = next_state.clone();
    Ok(next_state)
}

#[tauri::command]
fn evaluate_example(
    project: Project,
    sample_name: String,
    sample_folders: Vec<String>,
) -> Result<serde_json::Value, String> {
    engine::evaluate_example(&project, &sample_name, &sample_folders)
}

#[tauri::command]
async fn create_plan(state: State<'_, AppState>, project_ids: Vec<String>) -> Result<Plan, String> {
    if state.running.load(Ordering::Acquire) {
        return Err("다른 이동 작업이 실행 중입니다.".into());
    }
    let settings = state
        .settings
        .lock()
        .map_err(|_| "설정 잠금이 손상되었습니다.".to_string())?
        .clone();
    let internal =
        tauri::async_runtime::spawn_blocking(move || engine::create_plan(&settings, &project_ids))
            .await
            .map_err(|error| error.to_string())??;
    let public = internal.public.clone();
    state
        .plans
        .lock()
        .map_err(|_| "계획 잠금이 손상되었습니다.".to_string())?
        .insert(public.plan_id.clone(), internal);
    Ok(public)
}

struct RunningGuard<'a>(&'a AtomicBool);
impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
async fn start_run(
    app: AppHandle,
    state: State<'_, AppState>,
    plan_id: String,
) -> Result<RunResult, String> {
    if state
        .running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("다른 이동 작업이 실행 중입니다.".into());
    }
    let _guard = RunningGuard(&state.running);
    let plan = state
        .plans
        .lock()
        .map_err(|_| "계획 잠금이 손상되었습니다.".to_string())?
        .remove(&plan_id)
        .ok_or("이동 계획이 없거나 만료되었습니다.")?;
    let revision = state
        .settings
        .lock()
        .map_err(|_| "설정 잠금이 손상되었습니다.".to_string())?
        .revision;
    if revision != plan.public.revision {
        return Err("미리보기 이후 설정이 변경되었습니다. 다시 미리보기 해주세요.".into());
    }
    let runs_dir = storage::data_dir(&app)?.join("runs");
    state.cancel.store(false, Ordering::Release);
    let cancel = Arc::clone(&state.cancel);
    let result = tauri::async_runtime::spawn_blocking(move || {
        engine::execute_plan(plan, &runs_dir, &cancel)
    })
    .await
    .map_err(|error| error.to_string())??;
    storage::save_history(&app, &result)?;
    Ok(result)
}

#[tauri::command]
fn cancel_run(state: State<AppState>) -> Result<(), String> {
    if !state.running.load(Ordering::Acquire) {
        return Err("실행 중인 이동 작업이 없습니다.".into());
    }
    state.cancel.store(true, Ordering::Release);
    Ok(())
}

#[tauri::command]
fn list_history(app: AppHandle) -> Result<Vec<RunResult>, String> {
    storage::load_history(&app)
}

#[tauri::command]
fn export_settings(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    let snapshot = state
        .settings
        .lock()
        .map_err(|_| "설정 잠금이 손상되었습니다.".to_string())?
        .clone();
    storage::export_settings(&app, &PathBuf::from(path), &snapshot)
}

fn unique_import_name(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    for index in 1..=9999 {
        let candidate = format!("{base} (가져옴 {index})");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    format!("{base} ({})", Uuid::new_v4())
}

fn unique_import_tag_name(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_lowercase()) {
        return base.to_string();
    }
    let short = base.graphemes(true).take(60).collect::<String>();
    for index in 1..=9999 {
        let candidate = format!("{short} (가져옴 {index})");
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("{short} ({})", Uuid::new_v4())
}

#[tauri::command]
fn import_settings(
    app: AppHandle,
    state: State<AppState>,
    path: String,
    mode: String,
) -> Result<LocalState, String> {
    if state.running.load(Ordering::Acquire) {
        return Err("이동 작업 중에는 설정을 가져올 수 없습니다.".into());
    }
    let imported = storage::import_settings(&PathBuf::from(path))?;
    for project in &imported.projects {
        rules::validate_project(project)?;
    }
    validate_rule_tags(&imported.rule_tags)?;
    let mut current = state
        .settings
        .lock()
        .map_err(|_| "설정 잠금이 손상되었습니다.".to_string())?;
    let mut next = if mode == "replace" {
        LocalState {
            schema_version: 1,
            revision: current.revision + 1,
            preferences: imported.preferences,
            projects: imported.projects,
            rule_tags: imported.rule_tags,
        }
    } else if mode == "append" {
        let mut value = current.clone();
        let mut names: HashSet<String> = value
            .projects
            .iter()
            .map(|project| project.name.clone())
            .collect();
        for mut project in imported.projects {
            project.id = Uuid::new_v4().to_string();
            project.checked = false;
            project.name = unique_import_name(&project.name, &mut names);
            value.projects.push(project);
        }
        let mut source_names = HashSet::new();
        let mut target_names = HashSet::new();
        for tag in &value.rule_tags {
            let names = if tag.kind() == "source" {
                &mut source_names
            } else {
                &mut target_names
            };
            names.insert(tag.name().to_lowercase());
        }
        for mut tag in imported.rule_tags {
            tag.set_id(Uuid::new_v4().to_string());
            let names = if tag.kind() == "source" {
                &mut source_names
            } else {
                &mut target_names
            };
            tag.set_name(unique_import_tag_name(tag.name(), names));
            value.rule_tags.push(tag);
        }
        value.revision += 1;
        value
    } else {
        return Err("가져오기 방식이 올바르지 않습니다.".into());
    };
    next.schema_version = 1;
    validate_rule_tags(&next.rule_tags)?;
    storage::write_state(&app, &next)?;
    *current = next.clone();
    Ok(next)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.maximize().map_err(std::io::Error::other)?;
            }
            let settings = storage::load_state(app.handle()).map_err(std::io::Error::other)?;
            app.manage(AppState {
                settings: Mutex::new(settings),
                plans: Mutex::new(HashMap::new()),
                running: AtomicBool::new(false),
                cancel: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            load_state,
            save_state,
            evaluate_example,
            create_plan,
            start_run,
            cancel_run,
            list_history,
            import_settings,
            export_settings
        ])
        .run(tauri::generate_context!())
        .expect("MoveMgr 실행 중 치명적인 오류가 발생했습니다.");
}
