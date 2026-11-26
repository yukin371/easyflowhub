//! Command line for the spike.
//!
//! Hand-rolled rather than clap: the spike ships four modes and every dependency
//! added here has to be justified again in Phase 1.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Args {
    /// Run this many automated reveal cycles, then print the distribution.
    pub bench: Option<usize>,
    /// Pretend the panel lives on a monitor with this scale (100/150/200%).
    pub simulate_scale: Option<f32>,
    pub force_lite: bool,
    pub print_monitors: bool,
    /// Sample footprint (and idle CPU) and exit.
    pub probe: bool,
    pub idle_seconds: u64,
    pub settings_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
    pub budget_p95_ms: f64,
    pub budget_p99_ms: f64,
    pub visible_ms: Option<u64>,
    /// Show the panel once after warm-up and stay open (screenshot / manual check).
    pub demo: bool,
    /// Post this many hotkey messages 60 ms apart, to prove the debounce.
    pub burst: usize,
    /// Diagnostic: skip the CJK font so font/atlas cost can be measured separately.
    pub no_font: bool,
    /// Disable vsync — diagnostic only, it changes what the number means.
    pub no_vsync: bool,
    /// Draw the text island instead of the action grid (S2).
    pub editor: bool,
    /// Open the Phase 1 quick-note shell instead of the spike (batch 1a).
    pub note: bool,
    /// Open this note id in the quick-note shell instead of creating a new note.
    pub note_id: Option<String>,
    /// Run this many timed keystrokes in the island, then print the distribution.
    pub editor_bench: Option<usize>,
    /// S2 gate for "keystroke → character painted" (app side, excludes composition).
    pub editor_budget_p95_ms: f64,
    pub editor_budget_p99_ms: f64,
    /// Read (and with `store_write`, write) the product's `notes.db`, then exit (S3).
    pub store_bench: bool,
    /// App data directory to use instead of `%APPDATA%\com.yukin.easyflowhub`.
    pub store_root: Option<PathBuf>,
    /// Directory for the schema dumps and the row dump the S3 gate compares.
    pub store_artifacts: Option<PathBuf>,
    /// Allow `--store-bench` to write probe rows. Refused for the live app dir.
    pub store_write: bool,
    /// Hold a write transaction open for this many milliseconds, then commit (S3
    /// concurrency probe). Requires `--store-root`.
    pub store_hold_ms: Option<u64>,
    pub help: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            bench: None,
            simulate_scale: None,
            force_lite: false,
            print_monitors: false,
            probe: false,
            idle_seconds: 0,
            settings_path: None,
            log_path: None,
            budget_p95_ms: 60.0,
            budget_p99_ms: 100.0,
            visible_ms: None,
            demo: false,
            burst: 0,
            no_font: false,
            no_vsync: false,
            editor: false,
            note: false,
            note_id: None,
            editor_bench: None,
            editor_budget_p95_ms: 16.0,
            editor_budget_p99_ms: 25.0,
            store_bench: false,
            store_root: None,
            store_artifacts: None,
            store_write: false,
            store_hold_ms: None,
            help: false,
        }
    }
}

pub const USAGE: &str = "\
easyflowhub-native — S1 shell spike

  --bench <N>            run N reveal cycles and print the distribution
  --scale <1.0|1.5|2.0>  simulate a monitor scale (sets ppp and panel pixels)
  --lite                 force the lite profile
  --print-monitors       dump the cached monitor layout as JSON and exit
  --probe                sample footprint after warm-up and exit
  --idle-seconds <N>     with --probe: also report CPU time over N idle seconds
  --settings <path>      settings JSON (default: %APPDATA%\\com.yukin.easyflowhub\\native-spike.json)
  --log <path>           timing log (default: next to the settings file)
  --budget-p95 <ms>      gate threshold for p95 (default 60)
  --budget-p99 <ms>      gate threshold for p99 (default 100)
  --visible-ms <ms>      how long each bench reveal stays on screen (default 240)
  --demo                 open the panel once and stay open (screenshot aid)
  --burst <N>            post N hotkey messages 60 ms apart (debounce check)
  --no-font              diagnostic: skip the CJK font (font/atlas cost probe)
  --no-vsync             diagnostic: measure without vsync
  --editor               draw the RichEdit island instead of the action grid
  --note                 open the Phase 1 quick-note shell (title strip + island)
  --note-id <uuid>       open that note in the shell instead of a new one
  --editor-bench <N>     type N timed characters into the island, print, exit
  --editor-budget-p95 <ms>  gate for keystroke -> painted (default 16)
  --editor-budget-p99 <ms>  gate for keystroke -> painted p99 (default 25)
  --store-bench          read the product notes.db, print counts/schema, exit (S3)
  --store-root <dir>     app data dir to use instead of %APPDATA%
  --store-artifacts <dir>  write schema dumps + row dump there (S3 gate input)
  --store-write          let --store-bench write probe rows (refused for the live dir)
  --store-hold <ms>      hold a write transaction open that long, then commit
  -h, --help             this text
";

pub fn parse<I: IntoIterator<Item = String>>(raw: I) -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = raw.into_iter().skip(1);

    while let Some(arg) = iter.next() {
        let mut value = |name: &str| -> Result<String, String> {
            iter.next()
                .ok_or_else(|| format!("{name} needs a value; try --help"))
        };
        match arg.as_str() {
            "--bench" => {
                args.bench = Some(
                    value("--bench")?
                        .parse()
                        .map_err(|_| "--bench expects a number".to_string())?,
                );
            }
            "--scale" => {
                let parsed: f32 = value("--scale")?
                    .parse()
                    .map_err(|_| "--scale expects a number like 1.5".to_string())?;
                if !(0.5..=4.0).contains(&parsed) {
                    return Err("--scale out of range (0.5 - 4.0)".to_string());
                }
                args.simulate_scale = Some(parsed);
            }
            "--lite" => args.force_lite = true,
            "--print-monitors" => args.print_monitors = true,
            "--probe" => args.probe = true,
            "--idle-seconds" => {
                args.idle_seconds = value("--idle-seconds")?
                    .parse()
                    .map_err(|_| "--idle-seconds expects an integer".to_string())?;
            }
            "--settings" => args.settings_path = Some(PathBuf::from(value("--settings")?)),
            "--log" => args.log_path = Some(PathBuf::from(value("--log")?)),
            "--budget-p95" => {
                args.budget_p95_ms = value("--budget-p95")?
                    .parse()
                    .map_err(|_| "--budget-p95 expects milliseconds".to_string())?;
            }
            "--budget-p99" => {
                args.budget_p99_ms = value("--budget-p99")?
                    .parse()
                    .map_err(|_| "--budget-p99 expects milliseconds".to_string())?;
            }
            "--visible-ms" => {
                args.visible_ms = Some(
                    value("--visible-ms")?
                        .parse()
                        .map_err(|_| "--visible-ms expects an integer".to_string())?,
                );
            }
            "--demo" => args.demo = true,
            "--burst" => {
                args.burst = value("--burst")?
                    .parse()
                    .map_err(|_| "--burst expects a number".to_string())?;
            }
            "--no-font" => args.no_font = true,
            "--no-vsync" => args.no_vsync = true,
            "--editor" => args.editor = true,
            "--note" => args.note = true,
            "--note-id" => args.note_id = Some(value("--note-id")?),
            "--editor-budget-p95" => {
                args.editor_budget_p95_ms = value("--editor-budget-p95")?
                    .parse()
                    .map_err(|_| "--editor-budget-p95 expects milliseconds".to_string())?;
            }
            "--editor-budget-p99" => {
                args.editor_budget_p99_ms = value("--editor-budget-p99")?
                    .parse()
                    .map_err(|_| "--editor-budget-p99 expects milliseconds".to_string())?;
            }
            "--editor-bench" => {
                args.editor_bench = Some(
                    value("--editor-bench")?
                        .parse()
                        .map_err(|_| "--editor-bench expects a number".to_string())?,
                );
                // The bench needs the island; asking for one implies the other.
                args.editor = true;
            }
            "--store-bench" => args.store_bench = true,
            "--store-root" => args.store_root = Some(PathBuf::from(value("--store-root")?)),
            "--store-artifacts" => {
                args.store_artifacts = Some(PathBuf::from(value("--store-artifacts")?));
            }
            "--store-write" => args.store_write = true,
            "--store-hold" => {
                args.store_hold_ms = Some(
                    value("--store-hold")?
                        .parse()
                        .map_err(|_| "--store-hold expects milliseconds".to_string())?,
                );
            }
            "-h" | "--help" => args.help = true,
            other => return Err(format!("unknown argument '{other}'; try --help")),
        }
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(items: &[&str]) -> Result<Args, String> {
        parse(items.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_are_the_documented_gate() {
        let args = parse_args(&["app"]).unwrap();
        assert_eq!(args.budget_p95_ms, 60.0);
        assert_eq!(args.budget_p99_ms, 100.0);
        assert!(args.bench.is_none());
    }

    #[test]
    fn parses_a_bench_run() {
        let args = parse_args(&["app", "--bench", "200", "--scale", "1.5"]).unwrap();
        assert_eq!(args.bench, Some(200));
        assert_eq!(args.simulate_scale, Some(1.5));
    }

    #[test]
    fn rejects_a_missing_value() {
        assert!(parse_args(&["app", "--bench"]).is_err());
    }

    #[test]
    fn rejects_an_out_of_range_scale() {
        assert!(parse_args(&["app", "--scale", "9"]).is_err());
    }

    #[test]
    fn editor_bench_implies_the_editor() {
        let args = parse_args(&["app", "--editor-bench", "200"]).unwrap();
        assert_eq!(args.editor_bench, Some(200));
        assert!(args.editor);
    }

    #[test]
    fn note_mode_is_off_unless_asked_for() {
        assert!(!parse_args(&["app"]).unwrap().note);
        assert!(parse_args(&["app", "--note"]).unwrap().note);
    }

    #[test]
    fn rejects_unknown_flags() {
        assert!(parse_args(&["app", "--nope"]).is_err());
    }
}
