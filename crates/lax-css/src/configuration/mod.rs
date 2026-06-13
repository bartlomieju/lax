use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::configuration::NewLineKind;
use dprint_core::configuration::RECOMMENDED_GLOBAL_CONFIGURATION;
use dprint_core::configuration::ResolveConfigurationResult;
use dprint_core::configuration::get_unknown_property_diagnostics;
use dprint_core::configuration::get_value;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
  pub line_width: u32,
  pub use_tabs: bool,
  pub indent_width: u8,
  pub new_line_kind: NewLineKind,
  pub ignore_node_comment_text: String,
  pub ignore_file_comment_text: String,
  /// Format the input as a single line of declarations rather than a block.
  /// Used for inline `style=""` attributes, where line breaks are not
  /// allowed. Whitespace around colons and between declarations is
  /// normalized, but no declarations are wrapped or split onto new lines.
  #[serde(default)]
  pub single_line: bool,
}

pub fn resolve_config(
  config: ConfigKeyMap,
  global_config: &GlobalConfiguration,
) -> ResolveConfigurationResult<Configuration> {
  let mut config = config;
  let mut diagnostics = Vec::new();
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
    single_line: get_value(&mut config, "singleLine", false, &mut diagnostics),
  };
  diagnostics.extend(get_unknown_property_diagnostics(config));
  ResolveConfigurationResult {
    config: resolved_config,
    diagnostics,
  }
}
