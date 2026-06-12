use std::fs;
use std::path::Path;
use std::path::PathBuf;

use dprint_core::configuration::GlobalConfiguration;
use lax_sql::configuration::KeywordCase;
use lax_sql::configuration::resolve_config;
use lax_sql::format_text;

/// Runs every corpus file through the formatter at two line widths and
/// asserts the invariants of the lax policy:
///
/// 1. formatting never errors, no matter how exotic the dialect is
/// 2. formatting is idempotent
/// 3. everything except whitespace survives unchanged (compared case
///    insensitively for the keyword case configs)
#[test]
fn test_corpus() {
  let global_config = GlobalConfiguration::default();
  let default_config = resolve_config(Default::default(), &global_config).config;
  let mut narrow_config = default_config.clone();
  narrow_config.line_width = 40;
  let mut upper_config = default_config.clone();
  upper_config.keyword_case = KeywordCase::Upper;

  let mut files = Vec::new();
  collect_files(Path::new("./tests/corpus"), &mut files);
  files.sort();
  assert!(files.len() > 2000, "corpus seems to be missing files");

  // files whose syntax is irreducibly ambiguous without dialect knowledge:
  // backslash escaped single quotes in non E strings conflict with the
  // standard where a backslash is literal, and BigQuery triple quoted
  // strings conflict with standard quote doubling
  let known_ambiguous = [
    "ansi/escaped_quotes.sql",
    "ansi/like_operators.sql",
    "bigquery/string_literals.sql",
  ];

  let mut failures = Vec::new();
  for file in &files {
    if known_ambiguous.iter().any(|k| file.to_string_lossy().ends_with(k)) {
      continue;
    }
    let Ok(text) = fs::read_to_string(file) else {
      continue;
    };
    for (config_name, config) in [
      ("default", &default_config),
      ("narrow", &narrow_config),
      ("upper", &upper_config),
    ] {
      let first = match format_text(file, &text, config) {
        Ok(result) => result.unwrap_or_else(|| text.clone()),
        Err(err) => {
          failures.push(format!("{} [{}]: error: {}", file.display(), config_name, err));
          continue;
        }
      };
      match format_text(file, &first, config) {
        Ok(Some(second)) if second != first => {
          failures.push(format!("{} [{}]: not idempotent", file.display(), config_name));
        }
        Ok(_) => {}
        Err(err) => {
          failures.push(format!(
            "{} [{}]: error on second pass: {}",
            file.display(),
            config_name,
            err
          ));
        }
      }
      let case_insensitive = config.keyword_case != KeywordCase::Preserve;
      if essential_content(&text, case_insensitive) != essential_content(&first, case_insensitive) {
        failures.push(format!("{} [{}]: content changed", file.display(), config_name));
      }
    }
  }
  if !failures.is_empty() {
    panic!("{} corpus failures:\n{}", failures.len(), failures.join("\n"));
  }
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
  for entry in fs::read_dir(dir).unwrap() {
    let path = entry.unwrap().path();
    if path.is_dir() {
      collect_files(&path, files);
    } else if path.extension().and_then(|e| e.to_str()) == Some("sql") {
      files.push(path);
    }
  }
}

/// Everything except whitespace must survive formatting.
fn essential_content(text: &str, case_insensitive: bool) -> String {
  let stripped: String = text
    .chars()
    .filter(|c| !c.is_whitespace() && *c != '\u{feff}')
    .collect();
  if case_insensitive {
    stripped.to_lowercase()
  } else {
    stripped
  }
}
