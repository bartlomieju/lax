use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::generate_plugin_code;
use dprint_core::plugins::CheckConfigUpdatesMessage;
use dprint_core::plugins::ConfigChange;
use dprint_core::plugins::FileMatchingInfo;
use dprint_core::plugins::FormatError;
use dprint_core::plugins::FormatResult;
use dprint_core::plugins::PluginInfo;
use dprint_core::plugins::PluginResolveConfigurationResult;
use dprint_core::plugins::SyncFormatRequest;
use dprint_core::plugins::SyncHostFormatRequest;
use dprint_core::plugins::SyncPluginHandler;

use crate::configuration::Configuration;
use crate::configuration::resolve_config;

struct CssPluginHandler;

impl SyncPluginHandler<Configuration> for CssPluginHandler {
  fn resolve_config(
    &mut self,
    config: ConfigKeyMap,
    global_config: &GlobalConfiguration,
  ) -> PluginResolveConfigurationResult<Configuration> {
    let result = resolve_config(config, global_config);
    PluginResolveConfigurationResult {
      config: result.config,
      diagnostics: result.diagnostics,
      file_matching: FileMatchingInfo {
        file_extensions: vec!["css".to_string(), "scss".to_string(), "less".to_string()],
        file_names: vec![],
      },
    }
  }

  fn check_config_updates(&self, _message: CheckConfigUpdatesMessage) -> Result<Vec<ConfigChange>, FormatError> {
    Ok(Vec::new())
  }

  fn plugin_info(&mut self) -> PluginInfo {
    let version = env!("CARGO_PKG_VERSION").to_string();
    PluginInfo {
      name: env!("CARGO_PKG_NAME").to_string(),
      version: version.clone(),
      config_key: "css".to_string(),
      help_url: "https://github.com/bartlomieju/dprint-plugin-css".to_string(),
      config_schema_url: format!(
        "https://plugins.dprint.dev/bartlomieju/dprint-plugin-css/{}/schema.json",
        version
      ),
      update_url: Some("https://plugins.dprint.dev/bartlomieju/dprint-plugin-css/latest.json".to_string()),
    }
  }

  fn license_text(&mut self) -> String {
    include_str!("../LICENSE").to_string()
  }

  fn format(
    &mut self,
    request: SyncFormatRequest<Configuration>,
    _format_with_host: impl FnMut(SyncHostFormatRequest) -> FormatResult,
  ) -> FormatResult {
    let file_text = String::from_utf8(request.file_bytes).map_err(FormatError::new)?;
    crate::format_text(request.file_path, &file_text, request.config)
      .map(|maybe_text| maybe_text.map(|text| text.into_bytes()))
      .map_err(FormatError::new)
  }
}

generate_plugin_code!(CssPluginHandler, CssPluginHandler);
