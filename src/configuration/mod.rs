use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::ConfigurationDiagnostic;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::configuration::NewLineKind;
use dprint_core::configuration::RECOMMENDED_GLOBAL_CONFIGURATION;
use dprint_core::configuration::ResolveConfigurationResult;
use dprint_core::configuration::get_unknown_property_diagnostics;
use dprint_core::configuration::get_value;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeywordCase {
  /// Keywords are kept exactly as the author wrote them. The default.
  Preserve,
  /// Known SQL keywords are uppercased.
  Upper,
  /// Known SQL keywords are lowercased.
  Lower,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
  pub line_width: u32,
  pub use_tabs: bool,
  pub indent_width: u8,
  pub new_line_kind: NewLineKind,
  pub keyword_case: KeywordCase,
  pub ignore_node_comment_text: String,
  pub ignore_file_comment_text: String,
}

pub fn resolve_config(
  config: ConfigKeyMap,
  global_config: &GlobalConfiguration,
) -> ResolveConfigurationResult<Configuration> {
  let mut config = config;
  let mut diagnostics = Vec::new();
  let keyword_case_text: String = get_value(&mut config, "keywordCase", "preserve".to_string(), &mut diagnostics);
  let keyword_case = match keyword_case_text.as_str() {
    "preserve" => KeywordCase::Preserve,
    "upper" => KeywordCase::Upper,
    "lower" => KeywordCase::Lower,
    _ => {
      diagnostics.push(ConfigurationDiagnostic {
        property_name: "keywordCase".to_string(),
        message: format!(
          "expected \"preserve\", \"upper\", or \"lower\", but found \"{}\"",
          keyword_case_text
        ),
      });
      KeywordCase::Preserve
    }
  };
  let resolved_config = Configuration {
    line_width: get_value(
      &mut config,
      "lineWidth",
      global_config
        .line_width
        .unwrap_or(RECOMMENDED_GLOBAL_CONFIGURATION.line_width),
      &mut diagnostics,
    ),
    use_tabs: get_value(
      &mut config,
      "useTabs",
      global_config
        .use_tabs
        .unwrap_or(RECOMMENDED_GLOBAL_CONFIGURATION.use_tabs),
      &mut diagnostics,
    ),
    indent_width: get_value(
      &mut config,
      "indentWidth",
      global_config
        .indent_width
        .unwrap_or(RECOMMENDED_GLOBAL_CONFIGURATION.indent_width),
      &mut diagnostics,
    ),
    new_line_kind: get_value(
      &mut config,
      "newLineKind",
      global_config
        .new_line_kind
        .unwrap_or(RECOMMENDED_GLOBAL_CONFIGURATION.new_line_kind),
      &mut diagnostics,
    ),
    keyword_case,
    ignore_node_comment_text: get_value(
      &mut config,
      "ignoreNodeCommentText",
      "dprint-ignore".to_string(),
      &mut diagnostics,
    ),
    ignore_file_comment_text: get_value(
      &mut config,
      "ignoreFileCommentText",
      "dprint-ignore-file".to_string(),
      &mut diagnostics,
    ),
  };
  diagnostics.extend(get_unknown_property_diagnostics(config));
  ResolveConfigurationResult {
    config: resolved_config,
    diagnostics,
  }
}
