use std::path::Path;

fn main() {
  let path = std::env::args().nth(1).expect("usage: format <file>");
  let text = std::fs::read_to_string(&path).expect("failed to read file");
  let config_map = dprint_core::configuration::ConfigKeyMap::new();
  let global_config = dprint_core::configuration::GlobalConfiguration::default();
  let mut config = lax_sql::configuration::resolve_config(config_map, &global_config).config;
  if let Some(w) = std::env::args().nth(2) {
    config.line_width = w.parse().unwrap();
  }
  if std::env::args().nth(3).as_deref() == Some("expanded") {
    config.clause_style = lax_sql::configuration::ClauseStyle::Expanded;
  }
  match lax_sql::format_text(Path::new(&path), &text, &config) {
    Ok(Some(output)) => print!("{}", output),
    Ok(None) => print!("{}", text),
    Err(err) => {
      eprintln!("error: {}", err);
      std::process::exit(1);
    }
  }
}
