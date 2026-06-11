use std::path::Path;

fn main() {
  let path = std::env::args().nth(1).unwrap();
  let width: u32 = std::env::args().nth(2).unwrap().parse().unwrap();
  let global = dprint_core::configuration::GlobalConfiguration::default();
  let mut config = lax_sql::configuration::resolve_config(Default::default(), &global).config;
  config.line_width = width;
  let text = std::fs::read_to_string(&path).unwrap();
  let first = lax_sql::format_text(Path::new(&path), &text, &config)
    .unwrap()
    .unwrap_or_else(|| text.clone());
  match lax_sql::format_text(Path::new(&path), &first, &config).unwrap() {
    Some(second) if second != first => {
      eprintln!("NOT IDEMPOTENT");
      for (a, b) in first.lines().zip(second.lines()) {
        if a != b {
          eprintln!("first:  {:?}", a);
          eprintln!("second: {:?}", b);
        }
      }
    }
    _ => eprintln!("idempotent"),
  }
}
