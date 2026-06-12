use std::path::Path;

use dprint_core::configuration::GlobalConfiguration;
use lax_markup::configuration::resolve_config;
use lax_markup::format_text_with_external;

fn config() -> lax_markup::configuration::Configuration {
  resolve_config(Default::default(), &GlobalConfiguration::default()).config
}

#[test]
fn formats_embedded_blocks() {
  let input = "<html>\n<head>\n<style lang=\"scss\">\n.a{color:red}\n</style>\n<script type=\"module\">\nconst x=1\n</script>\n</head>\n</html>\n";
  let external = |lang: &str, text: &str, _width: u32| Ok(Some(format!("/* {} */\n{}", lang, text.trim())));
  let result = format_text_with_external(Path::new("a.html"), input, &config(), &external)
    .unwrap()
    .unwrap();
  let expected = "<html>\n  <head>\n    <style lang=\"scss\">\n    /* scss */\n    .a{color:red}\n    </style>\n    <script type=\"module\">\n    /* module */\n    const x=1\n    </script>\n  </head>\n</html>\n";
  assert_eq!(result, expected);
}

#[test]
fn external_decline_keeps_contents_verbatim() {
  let input = "<div>\n<style>\n  .a{}\n</style>\n</div>\n";
  let external = |_: &str, _: &str, _: u32| Ok(None);
  let result = format_text_with_external(Path::new("a.html"), input, &config(), &external).unwrap();
  assert_eq!(result.unwrap(), "<div>\n  <style>\n  .a{}\n</style>\n</div>\n");
}

#[test]
fn external_error_propagates() {
  let input = "<style>\nbad
</style>\n";
  let external = |_: &str, _: &str, _: u32| anyhow::bail!("syntax error at line 1");
  let result = format_text_with_external(Path::new("a.html"), input, &config(), &external);
  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("syntax error"));
}

#[test]
fn pre_is_never_sent_to_the_external_formatter() {
  let input = "<div>\n<pre>\n  keep   me\n</pre>\n</div>\n";
  let external = |_: &str, _: &str, _: u32| Ok(Some("CLOBBERED".to_string()));
  let result = format_text_with_external(Path::new("a.html"), input, &config(), &external)
    .unwrap()
    .unwrap();
  assert!(result.contains("keep   me"));
  assert!(!result.contains("CLOBBERED"));
}

#[test]
fn astro_frontmatter_is_formatted_as_typescript() {
  let input = "  ---\n// foo\n---\n<html></html>\n";
  let external = |lang: &str, text: &str, _: u32| {
    assert_eq!(lang, "ts");
    Ok(Some(format!("{}\n", text.trim())))
  };
  let result = format_text_with_external(Path::new("file.astro"), input, &config(), &external)
    .unwrap()
    .unwrap();
  assert_eq!(result, "---\n// foo\n---\n<html></html>\n");
}

#[test]
fn astro_without_frontmatter_formats_normally() {
  let input = "<html>\n<body></body>\n</html>\n";
  let external = |_: &str, _: &str, _: u32| Ok(None);
  let result = format_text_with_external(Path::new("file.astro"), input, &config(), &external).unwrap();
  assert_eq!(result.unwrap(), "<html>\n  <body></body>\n</html>\n");
}
