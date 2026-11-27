//! Composition root of the S1 spike.
//!
//! It owns the event loop, wires `hub-ui` to `hub-win`, and holds the one piece of
//! logic that must not live in either crate: the press handler, which knows both
//! the cached monitor layout (`hub-win`) and the clamping maths (`hub-core`).
//!
//! That handler runs **on the hotkey thread**, not on the paint thread. Splitting it
//! this way is what keeps the reveal path free of geometry work: by the time the UI
//! thread wakes up, the target rectangle is already known.

mod args;
mod backup;
mod bench;
mod click;
mod editor_bench;
mod esc;
mod font;
mod hub;
mod keys;
mod launcher;
mod log;
mod manager;
mod note;
mod probe_json;
mod save;
mod spike;
mod store_bench;
mod translate;
mod wheel;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hub_core::{PanelSpec, SpikeSettings};
use hub_win::MonitorCache;

use crate::log::Log;
use crate::spike::{Bus, Config, SpikeApp};

fn main() {
    // Must precede the first cursor/monitor read: without it Windows virtualises
    // coordinates for the whole process, so a 3840x2160 @150% screen reports
    // 2560x1440 and every placement lands at two thirds of the intended position.
    let aware = hub_win::dpi::make_per_monitor_aware();

    let parsed = match args::parse(std::env::args()) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(64);
        }
    };
    if parsed.help {
        print!("{}", args::USAGE);
        return;
    }

    let settings_path = parsed
        .settings_path
        .clone()
        .unwrap_or_else(default_settings_path);
    let (settings, source) = SpikeSettings::load(&settings_path);

    // S3 runs without a window: the question is about a file, and opening the
    // event loop first would make "reads the database" depend on a GPU.
    if let Some(hold_ms) = parsed.store_hold_ms {
        let root = match parsed.store_root.clone() {
            Some(root) => root,
            None => {
                eprintln!("--store-hold needs --store-root: it must never lock the live database");
                std::process::exit(64);
            }
        };
        std::process::exit(store_bench::hold_write_transaction(&root, hold_ms));
    }

    if parsed.store_bench {
        let code = store_bench::run(
            parsed.store_root.as_deref(),
            parsed.store_artifacts.as_deref(),
            parsed.store_write,
        );
        std::process::exit(code);
    }

    if parsed.print_monitors {
        let cache = MonitorCache::refresh();
        println!("{}", monitors_json(&cache));
        return;
    }

    let monitors = Arc::new(Mutex::new(MonitorCache::refresh()));
    let remote = hub_win::is_remote_session();
    let lite = parsed.force_lite || settings.lite || remote;

    if parsed.probe {
        spawn_probe_reporter(parsed.idle_seconds);
    }

    // The panel is sized in physical pixels (§5.1.1: the buffer is allocated per
    // panel, and at 200% a 440x860 logical panel is 1.5 Mpx).
    let scale = parsed
        .simulate_scale
        .unwrap_or_else(|| primary_scale(&monitors));
    // The panel is sized in physical pixels (§5.1.1: the buffer is allocated per
    // panel, and at 200% a 440x860 logical panel is 1.5 Mpx). Built before the
    // branch because the product process owns the launcher now.
    let spec = PanelSpec {
        desired_width: settings.panel_width * scale,
        desired_height: settings.panel_height * scale,
        margin: settings.margin,
        scale,
        ..PanelSpec::default()
    };
    // Phase 1: the quick note is its own process model now -- a parked controller
    // window owns the event loop and every note is a viewport of it (batch 1b.2),
    // so closing one note no longer ends the process by accident. `--resident`
    // comes through this same branch and skips only the first spawn.
    if parsed.note {
        // Claimed before the log exists, and that order is load-bearing: `Log::new`
        // truncates its file, so a second instance that opened the log first would
        // erase the first one's evidence and only then discover it had nothing to do.
        // It is also before the store, the chords and the tray -- everything a second
        // shell would have to undo.
        let instance = match hub_win::instance::claim() {
            hub_win::instance::Claim::First(guard) => guard,
            hub_win::instance::Claim::Taken => forward(parsed.autostart),
        };
        let log_path = parsed
            .log_path
            .clone()
            .unwrap_or_else(|| settings_path.with_file_name("native-note.log"));
        let log = std::sync::Arc::new(Log::new(log_path));
        // Without this line "my appearance settings did nothing" is undecidable:
        // the note falls back to defaults on an unreadable file, and a typo'd path
        // looks exactly like a theme that refuses to apply.
        log.mark(
            0,
            "settings",
            &format!(
                "path={} source={}",
                settings_path.display(),
                source.as_str()
            ),
        );
        // A null mutex is handle exhaustion, not a rival shell: the process runs and
        // says so, because refusing to start would be the worse failure.
        log.mark(
            0,
            "instance",
            &format!("claimed=true exclusive={}", instance.owned()),
        );
        hub::run(
            hub::Config {
                settings,
                scale,
                lite,
                log,
                store_root: parsed.store_root.clone(),
                note_id: parsed.note_id.clone(),
                resident: parsed.resident,
                settings_path: settings_path.clone(),
                no_font: parsed.no_font,
                font_heap: parsed.font_heap,
                launcher: launcher::Wiring {
                    monitors,
                    spec,
                    bench: parsed.launcher_bench,
                    budget_p95_ms: parsed.budget_p95_ms,
                    budget_p99_ms: parsed.budget_p99_ms,
                },
            },
            instance,
        );
    }

    let log_path = parsed
        .log_path
        .clone()
        .unwrap_or_else(|| settings_path.with_file_name("native-spike.log"));
    let log = Arc::new(Log::new(log_path));
    log.mark(
        0,
        "start",
        &format!(
            "settings={} source={} lite={} remote={remote} scale={scale:.2} vsync={} bench={:?}",
            settings_path.display(),
            source.as_str(),
            lite,
            !parsed.no_vsync,
            parsed.bench
        ),
    );
    log.mark(0, "log_path", &format!("{}", log.path().display()));
    log.mark(
        0,
        "dpi",
        &format!(
            "per_monitor_v2={aware} system_dpi={}",
            hub_win::dpi::system_dpi()
        ),
    );

    let bus = Bus::new();

    let config = Config {
        settings: settings.clone(),
        spec,
        scale,
        lite,
        monitors,
        bus: bus.clone(),
        log: log.clone(),
        budget_p95_ms: parsed.budget_p95_ms,
        budget_p99_ms: parsed.budget_p99_ms,
        skip_font: parsed.no_font,
        font_heap: parsed.font_heap,
        demo: parsed.demo,
        burst: parsed.burst,
        editor: parsed.editor,
        editor_bench: parsed.editor_bench,
        editor_budget_p95_ms: parsed.editor_budget_p95_ms,
        editor_budget_p99_ms: parsed.editor_budget_p99_ms,
    };
    let bench_total = parsed.bench;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(spike::WINDOW_TITLE)
            .with_inner_size([settings.panel_width, settings.panel_height])
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top(),
        vsync: !parsed.no_vsync,
        ..Default::default()
    };

    let result = eframe::run_native(
        spike::WINDOW_TITLE,
        options,
        Box::new(move |cc| {
            bus.attach(cc.egui_ctx.clone());
            let app: Box<dyn eframe::App> = Box::new(SpikeApp::new(cc, config, bench_total));
            Ok(app)
        }),
    );

    if let Err(error) = result {
        log.mark(0, "fatal", &format!("{error}"));
        eprintln!("{error}");
        std::process::exit(1);
    }
}

/// Hand the launch to the shell that already owns it, then leave.
///
/// Printed rather than logged on purpose: this process does not own the log, and the
/// whole point is that it touches no shared state before exiting. The evidence that
/// the handover worked is the owner's line (`instance action=reveal`), written by the
/// process that can still see the panel.
fn forward(autostart: bool) -> ! {
    if autostart {
        // The login entry's second instance. Windows starts it while the desktop is
        // still settling and the user has not asked for anything yet, so summoning
        // the panel here would be a window appearing over the login for no reason.
        eprintln!("instance action=quiet reason=autostart");
        std::process::exit(0);
    }
    match hub_win::instance::signal() {
        Ok(()) => eprintln!("instance action=forward"),
        // The owner died between our claim attempt and the ring. Nothing to show the
        // user: the panel they wanted does not exist any more, and starting a shell
        // from inside the failure path is the bug, not the fix.
        Err(error) => eprintln!("instance action=give-up error=\"{error}\""),
    }
    std::process::exit(0);
}

fn primary_scale(monitors: &Arc<Mutex<MonitorCache>>) -> f32 {
    let cache = monitors.lock().unwrap();
    let cursor = hub_win::cursor_pos().unwrap_or((0, 0));
    cache.at(cursor.0, cursor.1).scale
}

fn default_settings_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base)
        .join("com.yukin.easyflowhub")
        .join("native-spike.json")
}

fn monitors_json(cache: &MonitorCache) -> String {
    let entries: Vec<String> = cache
        .as_slice()
        .iter()
        .map(|monitor| {
            format!(
                "{{\"name\":{},\"rect\":[{},{},{},{}],\"work\":[{},{},{},{}],\"scale\":{:.2},\"primary\":{}}}",
                serde_json::to_string(&monitor.name).unwrap_or_else(|_| "\"\"".to_string()),
                monitor.rect.0,
                monitor.rect.1,
                monitor.rect.2,
                monitor.rect.3,
                monitor.work.0,
                monitor.work.1,
                monitor.work.2,
                monitor.work.3,
                monitor.scale,
                monitor.primary
            )
        })
        .collect();
    format!(
        "{{\"kind\":\"native-s1-monitors\",\"remote\":{},\"count\":{},\"monitors\":[{}]}}",
        hub_win::is_remote_session(),
        cache.len(),
        entries.join(",")
    )
}

/// `--probe`: wait for the warm frames, snapshot, optionally measure idle CPU, exit.
fn spawn_probe_reporter(idle_seconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        let cpu_before = hub_win::probe::cpu_time_ms();
        let first = probe_json::ProbeJson::capture(0.0);

        if idle_seconds > 0 {
            std::thread::sleep(Duration::from_secs(idle_seconds));
        }
        let idle_cpu_ms = hub_win::probe::cpu_time_ms() - cpu_before;
        let last = probe_json::ProbeJson::capture(idle_cpu_ms);
        let windows = hub_win::window::top_level_windows();
        let visible = windows.iter().filter(|window| window.visible).count();
        let titles: Vec<String> = windows
            .iter()
            .map(|window| format!("{}:{}", window.title, if window.visible { "v" } else { "h" }))
            .collect();

        println!(
            "{{\"kind\":\"native-s1-probe\",\"private_mb\":{:.1},\"private_ws_mb\":{:.1},\"working_set_mb\":{:.1},\"atlas\":\"{}x{}\",\"atlas_mb\":{:.1},\"start_private_mb\":{:.1},\"handles\":{},\"cpu_ms\":{:.0},\"idle_cpu_ms\":{:.0},\"windows\":{},\"visible_windows\":{},\"window_titles\":{}}}",
            last.private_mb,
            last.private_ws_mb,
            last.working_set_mb,
            last.atlas_w,
            last.atlas_h,
            last.atlas_w as f64 * last.atlas_h as f64 / (1024.0 * 1024.0),
            first.private_mb,
            last.handles,
            last.cpu_ms,
            last.idle_cpu_ms,
            windows.len(),
            visible,
            serde_json::to_string(&titles).unwrap_or_else(|_| "[]".to_string())
        );
        std::process::exit(0);
    });
}

