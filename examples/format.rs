use std::path::Path;

fn main() {
  let path = std::env::args().nth(1).expect("usage: format <file>");
  let text = std::fs::read_to_string(&path).expect("failed to read file");
  let config_map = dprint_core::configuration::ConfigKeyMap::new();
  let global_config = dprint_core::configuration::GlobalConfiguration::default();
  let config = dprint_plugin_css::configuration::resolve_config(config_map, &global_config).config;
  match dprint_plugin_css::format_text(Path::new(&path), &text, &config) {
    Ok(Some(output)) => print!("{}", output),
    Ok(None) => print!("{}", text),
    Err(err) => {
      eprintln!("error: {}", err);
      std::process::exit(1);
    }
  }
}
