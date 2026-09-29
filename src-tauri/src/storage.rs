use crate::models::{LocalState, PortableSettings, RunResult};
use chrono::Utc;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::AppHandle;

fn portable_root() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("실행 파일 위치를 확인할 수 없습니다: {error}"))?;
    #[cfg(target_os = "macos")]
    let root = executable
        .ancestors()
        .nth(4)
        .ok_or_else(|| "macOS 앱 폴더 위치를 확인할 수 없습니다.".to_string())?;
    #[cfg(not(target_os = "macos"))]
    let root = executable
        .parent()
        .ok_or_else(|| "실행 파일 폴더를 확인할 수 없습니다.".to_string())?;
    Ok(root.to_path_buf())
}

pub fn config_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    Ok(portable_root()?.join("MoveMgrData"))
}

pub fn data_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    Ok(portable_root()?.join("MoveMgrData"))
}

pub fn load_state(app: &AppHandle) -> Result<LocalState, String> {
    let dir = config_dir(app)?;
    fs::create_dir_all(&dir).map_err(|error| format!("설정 폴더를 만들 수 없습니다: {error}"))?;
    let settings = dir.join("settings.json");
    if !settings.exists() {
        return Ok(LocalState::default());
    }
    match read_json(&settings) {
        Ok(state) => Ok(state),
        Err(primary) => {
            let backup = dir.join("settings.backup.json");
            if backup.exists() {
                read_json(&backup).map_err(|backup_error| {
                    format!("설정과 백업을 읽지 못했습니다. 설정: {primary}; 백업: {backup_error}")
                })
            } else {
                Err(primary)
            }
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn write_state(app: &AppHandle, state: &LocalState) -> Result<(), String> {
    let dir = config_dir(app)?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    atomic_json(
        &dir.join("settings.json"),
        &dir.join("settings.backup.json"),
        state,
    )
}

fn atomic_json<T: serde::Serialize>(path: &Path, backup: &Path, value: &T) -> Result<(), String> {
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    let _: serde_json::Value =
        serde_json::from_slice(&fs::read(&temp).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if path.exists() {
        let _ = fs::remove_file(backup);
        fs::rename(path, backup).map_err(|error| format!("기존 설정 백업 실패: {error}"))?;
    }
    if let Err(error) = fs::rename(&temp, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(backup, path);
        }
        return Err(format!("설정 교체 실패: {error}"));
    }
    Ok(())
}

pub fn export_settings(app: &AppHandle, path: &Path, state: &LocalState) -> Result<(), String> {
    let portable = PortableSettings {
        format: "movemgr-settings".into(),
        schema_version: 1,
        exported_by_app_version: app.package_info().version.to_string(),
        exported_at: Utc::now().to_rfc3339(),
        preferences: state.preferences.clone(),
        projects: state.projects.clone(),
        rule_tags: state.rule_tags.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&portable).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| format!("내보내기 실패: {error}"))
}

pub fn import_settings(path: &Path) -> Result<PortableSettings, String> {
    let meta =
        fs::metadata(path).map_err(|error| format!("가져올 파일을 읽을 수 없습니다: {error}"))?;
    if meta.len() > 10 * 1024 * 1024 {
        return Err("설정 파일은 10MiB를 넘을 수 없습니다.".into());
    }
    let value: PortableSettings = read_json(path)?;
    if value.format != "movemgr-settings" || value.schema_version != 1 {
        return Err("지원하지 않는 MoveMgr 설정 형식입니다.".into());
    }
    Ok(value)
}

pub fn save_history(app: &AppHandle, result: &RunResult) -> Result<(), String> {
    let dir = data_dir(app)?.join("runs").join(&result.run_id);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    fs::write(
        dir.join("summary.json"),
        serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

pub fn load_history(app: &AppHandle) -> Result<Vec<RunResult>, String> {
    let root = data_dir(app)?.join("runs");
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut items = vec![];
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let path = entry
            .map_err(|error| error.to_string())?
            .path()
            .join("summary.json");
        if path.exists() {
            if let Ok(value) = read_json::<RunResult>(&path) {
                items.push(value);
            }
        }
    }
    items.sort_by(|a, b| b.finished_at.cmp(&a.finished_at));
    items.truncate(100);
    Ok(items)
}
