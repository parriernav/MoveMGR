use crate::models::*;
use std::path::{Component, Path};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

pub fn normalize(value: &str, ignore_case: bool) -> String {
    let nfc: String = value.nfc().collect();
    if ignore_case {
        nfc.to_lowercase()
    } else {
        nfc
    }
}

pub fn visible_prefix(value: &str, count: usize) -> Option<String> {
    let normalized: String = value.nfc().collect();
    let parts: Vec<&str> = UnicodeSegmentation::graphemes(normalized.as_str(), true).collect();
    (parts.len() >= count && count > 0).then(|| parts[..count].concat())
}

pub fn split_file_name(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(index) if index > 0 && index + 1 < name.len() => {
            (&name[..index], Some(&name[index + 1..]))
        }
        _ => (name, None),
    }
}

pub fn extract(value: &str, extractor: &Extractor, trim: bool) -> Result<String, &'static str> {
    let result = match extractor {
        Extractor::Whole => value.nfc().collect(),
        Extractor::FirstChars { count } => visible_prefix(value, *count).ok_or("KEY_TOO_SHORT")?,
        Extractor::BeforeDelimiter { delimiter, missing } => {
            if delimiter.is_empty() {
                return Err("INVALID_CONFIG");
            }
            match value.find(delimiter) {
                Some(index) => value[..index].nfc().collect(),
                None if matches!(missing, MissingDelimiter::UseWhole) => value.nfc().collect(),
                None => return Err("DELIMITER_MISSING"),
            }
        }
    };
    let result = if trim {
        result.trim().to_string()
    } else {
        result
    };
    if result.is_empty() {
        Err("EMPTY_KEY")
    } else {
        Ok(result)
    }
}

pub fn file_matches(project: &Project, stem: &str, extension: Option<&str>) -> bool {
    let extension_ok = match &project.source.extensions {
        ExtensionFilter::All => true,
        ExtensionFilter::Only {
            values,
            include_extensionless,
        } => match extension {
            Some(extension) => values.iter().any(|value| {
                value
                    .trim_start_matches("*.")
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(extension)
            }),
            None => *include_extensionless,
        },
    };
    if !extension_ok {
        return false;
    }
    let haystack = normalize(stem, project.comparison_options.ignore_case);
    project.source.name_filters.iter().all(|filter| {
        if filter.value.is_empty() {
            return false;
        }
        let needle = normalize(&filter.value, project.comparison_options.ignore_case);
        match filter.op {
            NameFilterOp::StartsWith => haystack.starts_with(&needle),
            NameFilterOp::Contains => haystack.contains(&needle),
            NameFilterOp::EndsWith => haystack.ends_with(&needle),
            NameFilterOp::Equals => haystack == needle,
        }
    })
}

pub fn folder_matches(
    key: &str,
    folder_value: &str,
    comparison: &Comparison,
    ignore_case: bool,
) -> bool {
    let key = normalize(key, ignore_case);
    let folder = normalize(folder_value, ignore_case);
    match comparison {
        Comparison::Equals => key == folder,
        Comparison::StartsWith => folder.starts_with(&key),
        Comparison::PrefixEqual { count } => match (
            visible_prefix(&key, *count),
            visible_prefix(&folder, *count),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        },
    }
}

pub fn safe_relative(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

pub fn valid_key_folder(value: &str) -> bool {
    if value.is_empty() || value == "." || value == ".." || value.ends_with([' ', '.']) {
        return false;
    }
    if value.chars().any(|ch| {
        ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return false;
    }
    let upper = value.trim_end_matches([' ', '.']).to_uppercase();
    let base = upper.split('.').next().unwrap_or("");
    !matches!(
        base,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub fn validate_source_rule(
    extensions: &ExtensionFilter,
    name_filters: &[NameFilter],
) -> Result<(), String> {
    if let ExtensionFilter::Only { values, .. } = extensions {
        if values.is_empty() {
            return Err("확장자를 하나 이상 지정해주세요.".into());
        }
    }
    if name_filters
        .iter()
        .any(|filter| filter.value.trim().is_empty())
    {
        return Err("파일명 조건의 문자열이 비어 있습니다.".into());
    }
    Ok(())
}

fn validate_extractor(extractor: &Extractor) -> Result<(), String> {
    if let Extractor::FirstChars { count } = extractor {
        if *count == 0 {
            return Err("분류 글자 수는 1 이상이어야 합니다.".into());
        }
    }
    if let Extractor::BeforeDelimiter { delimiter, .. } = extractor {
        if delimiter.is_empty() {
            return Err("구분자가 비어 있습니다.".into());
        }
    }
    Ok(())
}

pub fn validate_target_rule(key: &KeyRule, destination: &Destination) -> Result<(), String> {
    validate_extractor(&key.extractor)?;
    match destination {
        Destination::FixedSubfolder { relative_path, .. } if !safe_relative(relative_path) => {
            return Err("고정 하위 폴더는 안전한 상대 경로여야 합니다.".into())
        }
        Destination::MatchSubfolder {
            search_base,
            folder_extractor,
            comparison,
            ..
        } => {
            if !safe_relative(search_base) {
                return Err("검색 기준 폴더는 안전한 상대 경로여야 합니다.".into());
            }
            validate_extractor(folder_extractor)?;
            if matches!(comparison, Comparison::PrefixEqual { count: 0 }) {
                return Err("비교 글자 수는 1 이상이어야 합니다.".into());
            }
        }
        Destination::KeySubfolder {
            parent_relative_path,
            ..
        } if !safe_relative(parent_relative_path) => {
            return Err("기준 하위 폴더는 안전한 상대 경로여야 합니다.".into())
        }
        _ => {}
    }
    Ok(())
}

pub fn validate_project(project: &Project) -> Result<(), String> {
    if project.name.trim().is_empty() {
        return Err("프로젝트 이름이 비어 있습니다.".into());
    }
    validate_source_rule(&project.source.extensions, &project.source.name_filters)?;
    validate_target_rule(&project.key, &project.target.destination)
}

pub fn validate_rule_tag(tag: &RuleTag) -> Result<(), String> {
    match tag {
        RuleTag::Source { rules, .. } => {
            validate_source_rule(&rules.extensions, &rules.name_filters)
        }
        RuleTag::Target { rules, .. } => validate_target_rule(&rules.key, &rules.destination),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_extensions_as_specified() {
        assert_eq!(split_file_name("a.tar.gz"), ("a.tar", Some("gz")));
        assert_eq!(split_file_name(".gitignore"), (".gitignore", None));
        assert_eq!(split_file_name("파일."), ("파일.", None));
    }

    #[test]
    fn grapheme_prefix_handles_composed_korean_and_emoji() {
        assert_eq!(visible_prefix("가나다", 2).as_deref(), Some("가나"));
        assert_eq!(visible_prefix("👨‍👩‍👧‍👦자료", 1).as_deref(), Some("👨‍👩‍👧‍👦"));
        assert!(visible_prefix("가", 2).is_none());
    }

    #[test]
    fn prefix_comparison_requires_both_values_long_enough() {
        assert!(folder_matches(
            "ABC2026",
            "ABC 고객",
            &Comparison::PrefixEqual { count: 3 },
            true
        ));
        assert!(!folder_matches(
            "AB",
            "ABC 고객",
            &Comparison::PrefixEqual { count: 3 },
            true
        ));
    }
}
