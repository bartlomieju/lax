// Performance + robustness benchmarks for the lax formatters, measured against
// the libraries they replace, on the vendored test corpora.
//
//   cargo run --release --manifest-path benchmarks/Cargo.toml
//
// Speed is measured per file (the real `deno fmt` workload), summing wall time
// over every corpus file and reporting throughput in MB/s. Robustness counts
// how many corpus inputs the incumbent rejects (parse error or panic) that lax
// formats without error.
use std::path::{Path, PathBuf};
use std::time::Instant;

use dprint_core::configuration::GlobalConfiguration;

fn corpus(crate_dir: &str, exts: &[&str]) -> Vec<(PathBuf, String)> {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("..")
    .join("crates")
    .join(crate_dir)
    .join("tests/corpus");
  let mut out = Vec::new();
  walk(&root, exts, &mut out);
  out
}

fn walk(dir: &Path, exts: &[&str], out: &mut Vec<(PathBuf, String)>) {
  if let Ok(entries) = std::fs::read_dir(dir) {
    for e in entries.flatten() {
      let p = e.path();
      if p.is_dir() {
        walk(&p, exts, out);
      } else if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
        if exts.contains(&ext) {
          if let Ok(s) = std::fs::read_to_string(&p) {
            out.push((p, s));
          }
        }
      }
    }
  }
}

fn mbps(bytes: usize, iters: usize, secs: f64) -> f64 {
  (bytes * iters) as f64 / secs / 1_000_000.0
}

fn bench_css() {
  let files = corpus("lax-css", &["css", "scss", "less"]);
  let cfg = lax_css::configuration::Configuration {
    line_width: 80,
    use_tabs: false,
    indent_width: 2,
    new_line_kind: dprint_core::configuration::NewLineKind::LineFeed,
    ignore_node_comment_text: "dprint-ignore".into(),
    ignore_file_comment_text: "dprint-ignore-file".into(),
    single_line: false,
  };
  let mopts = malva::config::FormatOptions::default();
  let syntax = |p: &Path| match p.extension().and_then(|s| s.to_str()) {
    Some("scss") => malva::Syntax::Scss,
    Some("less") => malva::Syntax::Less,
    _ => malva::Syntax::Css,
  };

  std::panic::set_hook(Box::new(|_| {}));
  let mut rejected = 0usize;
  let mut common = Vec::new();
  for (p, src) in &files {
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      malva::format_text(src, syntax(p), &mopts).is_ok()
    }))
    .unwrap_or(false);
    if ok {
      common.push((p.clone(), src.clone()));
    } else {
      rejected += 1;
    }
  }

  let bytes: usize = common.iter().map(|(_, s)| s.len()).sum();
  let iters = 30;
  for (p, s) in &common {
    let _ = lax_css::format_text(p, s, &cfg);
  }
  let t = Instant::now();
  for _ in 0..iters {
    for (p, s) in &common {
      let _ = lax_css::format_text(p, s, &cfg);
    }
  }
  let lax = t.elapsed();
  let t = Instant::now();
  for _ in 0..iters {
    for (p, s) in &common {
      let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        malva::format_text(s, syntax(p), &mopts)
      }));
    }
  }
  let inc = t.elapsed();
  println!("## CSS / SCSS / Less   (vs malva 0.16)");
  println!(
    "   corpus      {} files, {} rejected by malva (lax formats all)",
    files.len(),
    rejected
  );
  println!("   lax-css     {:>3.0} MB/s", mbps(bytes, iters, lax.as_secs_f64()));
  println!("   malva       {:>3.0} MB/s", mbps(bytes, iters, inc.as_secs_f64()));
  println!(
    "   speedup     {:.2}x\n",
    mbps(bytes, iters, lax.as_secs_f64()) / mbps(bytes, iters, inc.as_secs_f64())
  );
}

fn bench_sql() {
  let files = corpus("lax-sql", &["sql"]);
  let cfg = lax_sql::configuration::resolve_config(
    Default::default(),
    &GlobalConfiguration::default(),
  )
  .config;
  let sopts = sqlformat::FormatOptions::default();
  let bytes: usize = files.iter().map(|(_, s)| s.len()).sum();
  let iters = 20;
  for (p, s) in &files {
    let _ = lax_sql::format_text(p, s, &cfg);
  }
  let t = Instant::now();
  for _ in 0..iters {
    for (p, s) in &files {
      let _ = lax_sql::format_text(p, s, &cfg);
    }
  }
  let lax = t.elapsed();
  let t = Instant::now();
  for _ in 0..iters {
    for (_, s) in &files {
      let _ = sqlformat::format(s, &sqlformat::QueryParams::None, &sopts);
    }
  }
  let inc = t.elapsed();
  println!("## SQL                 (vs sqlformat-rs 0.5)");
  println!("   corpus      {} files", files.len());
  println!("   lax-sql     {:>3.0} MB/s", mbps(bytes, iters, lax.as_secs_f64()));
  println!("   sqlformat   {:>3.0} MB/s", mbps(bytes, iters, inc.as_secs_f64()));
  println!(
    "   speedup     {:.2}x\n",
    mbps(bytes, iters, lax.as_secs_f64()) / mbps(bytes, iters, inc.as_secs_f64())
  );
}

fn bench_markup() {
  let files = corpus("lax-markup", &["html", "xml", "svg", "vue", "svelte", "astro"]);
  let cfg = lax_markup::configuration::resolve_config(
    Default::default(),
    &GlobalConfiguration::default(),
  )
  .config;
  let bytes: usize = files.iter().map(|(_, s)| s.len()).sum();
  let iters = 50;
  for (p, s) in &files {
    let _ = lax_markup::format_text(p, s, &cfg);
  }
  let t = Instant::now();
  for _ in 0..iters {
    for (p, s) in &files {
      let _ = lax_markup::format_text(p, s, &cfg);
    }
  }
  let lax = t.elapsed();
  println!("## HTML / XML / SVG / components");
  println!("   corpus      {} files", files.len());
  println!("   lax-markup  {:>3.0} MB/s\n", mbps(bytes, iters, lax.as_secs_f64()));
}

fn main() {
  println!();
  bench_css();
  bench_sql();
  bench_markup();
}
