use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Extractor {
    Whole,
    FirstChars {
        count: usize,
    },
    BeforeDelimiter {
        delimiter: String,
        missing: MissingDelimiter,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MissingDelimiter {
    Skip,
    UseWhole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum ExtensionFilter {
    All,
    Only {
        values: Vec<String>,
        #[serde(rename = "includeExtensionless", alias = "include_extensionless")]
        include_extensionless: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameFilter {
    pub op: NameFilterOp,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NameFilterOp {
    StartsWith,
    Contains,
    EndsWith,
    Equals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Comparison {
    Equals,
    StartsWith,
    PrefixEqual { count: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum Destination {
    Root,
    FixedSubfolder {
        #[serde(rename = "relativePath", alias = "relative_path")]
        relative_path: String,
        #[serde(rename = "createIfMissing", alias = "create_if_missing")]
        create_if_missing: bool,
    },
    MatchSubfolder {
        #[serde(rename = "searchBase", alias = "search_base")]
        search_base: String,
        #[serde(rename = "folderExtractor", alias = "folder_extractor")]
        folder_extractor: Extractor,
        comparison: Comparison,
        #[serde(rename = "noMatch", alias = "no_match")]
        no_match: NoMatch,
    },
    KeySubfolder {
        #[serde(rename = "parentRelativePath", alias = "parent_relative_path")]
        parent_relative_path: String,
        #[serde(rename = "createIfMissing", alias = "create_if_missing")]
        create_if_missing: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoMatch {
    Skip,
    CreateKeyFolder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Conflict {
    Skip,
    RenameWithNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub root: String,
    pub recursive: bool,
    pub include_hidden: bool,
    pub extensions: ExtensionFilter,
    pub name_filters: Vec<NameFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyRule {
    pub extractor: Extractor,
    pub trim: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComparisonOptions {
    pub ignore_case: bool,
    pub normalization: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub root: String,
    pub destination: Destination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub checked: bool,
    pub source: Source,
    pub key: KeyRule,
    pub comparison_options: ComparisonOptions,
    pub target: Target,
    pub conflict: Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRuleSnapshot {
    pub recursive: bool,
    pub include_hidden: bool,
    pub extensions: ExtensionFilter,
    pub name_filters: Vec<NameFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetRuleSnapshot {
    pub key: KeyRule,
    pub comparison_options: ComparisonOptions,
    pub destination: Destination,
    pub conflict: Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RuleTag {
    Source {
        id: String,
        name: String,
        rules: SourceRuleSnapshot,
    },
    Target {
        id: String,
        name: String,
        rules: TargetRuleSnapshot,
    },
}

impl RuleTag {
    pub fn id(&self) -> &str {
        match self {
            Self::Source { id, .. } | Self::Target { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Source { name, .. } | Self::Target { name, .. } => name,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Source { .. } => "source",
            Self::Target { .. } => "target",
        }
    }

    pub fn set_id(&mut self, next_id: String) {
        match self {
            Self::Source { id, .. } | Self::Target { id, .. } => *id = next_id,
        }
    }

    pub fn set_name(&mut self, next_name: String) {
        match self {
            Self::Source { name, .. } | Self::Target { name, .. } => *name = next_name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub theme: String,
    pub language: String,
    pub preview_before_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalState {
    pub schema_version: u32,
    pub revision: u64,
    pub preferences: Preferences,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub rule_tags: Vec<RuleTag>,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            preferences: Preferences {
                theme: "system".into(),
                language: "en".into(),
                preview_before_run: true,
            },
            projects: vec![],
            rule_tags: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedItem {
    pub item_id: String,
    pub project_id: String,
    pub project_name: String,
    pub source_path: String,
    pub extracted_key: Option<String>,
    pub proposed_target_path: Option<String>,
    pub decision: String,
    pub reason_code: Option<String>,
    pub reason_text: Option<String>,
    pub size_bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub plan_id: String,
    pub created_at: String,
    pub revision: u64,
    pub items: Vec<PlannedItem>,
    pub movable: usize,
    pub skipped: usize,
    pub blocked: usize,
}

#[derive(Debug, Clone)]
pub struct InternalItem {
    pub public: PlannedItem,
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub len: u64,
    pub modified_nanos: u128,
    pub conflict: Conflict,
}

#[derive(Debug, Clone)]
pub struct InternalPlan {
    pub public: Plan,
    pub items: Vec<InternalItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunItem {
    #[serde(flatten)]
    pub planned: PlannedItem,
    pub state: String,
    pub final_target_path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub moved: usize,
    pub skipped: usize,
    pub failed: usize,
    pub source_retained: usize,
    pub items: Vec<RunItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub config_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableSettings {
    pub format: String,
    pub schema_version: u32,
    pub exported_by_app_version: String,
    pub exported_at: String,
    pub preferences: Preferences,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub rule_tags: Vec<RuleTag>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn destination_rules_accept_frontend_fields_and_serialize_them_back() {
        let cases = [
            json!({"mode":"fixedSubfolder","relativePath":"완료/2026","createIfMissing":true}),
            json!({"mode":"matchSubfolder","searchBase":"","folderExtractor":{"kind":"beforeDelimiter","delimiter":"-","missing":"skip"},"comparison":{"kind":"equals"},"noMatch":"skip"}),
            json!({"mode":"keySubfolder","parentRelativePath":"보관","createIfMissing":true}),
        ];
        for value in cases {
            let rule: Destination = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(rule).unwrap(), value);
        }
    }

    #[test]
    fn old_snake_case_rule_fields_remain_readable() {
        let old: Value = json!({"mode":"matchSubfolder","search_base":"","folder_extractor":{"kind":"whole"},"comparison":{"kind":"equals"},"no_match":"skip"});
        let rule: Destination = serde_json::from_value(old).unwrap();
        let current = serde_json::to_value(rule).unwrap();
        assert_eq!(current["searchBase"], "");
        assert_eq!(current["folderExtractor"]["kind"], "whole");
        assert_eq!(current["noMatch"], "skip");

        let extensions: ExtensionFilter = serde_json::from_value(
            json!({"mode":"only","values":["pdf"],"include_extensionless":false}),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(extensions).unwrap()["includeExtensionless"],
            false
        );
    }

    #[test]
    fn old_state_without_tags_loads_and_tagged_state_round_trips() {
        let old = json!({
            "schemaVersion": 1,
            "revision": 2,
            "preferences": {"theme":"system","language":"ko","previewBeforeRun":true},
            "projects": []
        });
        let mut state: LocalState = serde_json::from_value(old).unwrap();
        assert!(state.rule_tags.is_empty());
        state.rule_tags.push(RuleTag::Target {
            id: uuid::Uuid::new_v4().to_string(),
            name: "고객사별".into(),
            rules: TargetRuleSnapshot {
                key: KeyRule {
                    extractor: Extractor::BeforeDelimiter {
                        delimiter: "-".into(),
                        missing: MissingDelimiter::Skip,
                    },
                    trim: true,
                },
                comparison_options: ComparisonOptions {
                    ignore_case: true,
                    normalization: "NFC".into(),
                },
                destination: Destination::MatchSubfolder {
                    search_base: String::new(),
                    folder_extractor: Extractor::Whole,
                    comparison: Comparison::Equals,
                    no_match: NoMatch::Skip,
                },
                conflict: Conflict::Skip,
            },
        });
        let encoded = serde_json::to_value(&state).unwrap();
        assert_eq!(
            encoded["ruleTags"][0]["rules"]["destination"]["searchBase"],
            ""
        );
        let decoded: LocalState = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.rule_tags.len(), 1);
    }
}
