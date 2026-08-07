//! Local e2e capture probe: detection + desktop capture + structured report.
//!
//! Exit codes:
//! - 0: capture succeeded (and expect checks passed if provided)
//! - 1: capture or expect failed
//! - 2: soft skip (e.g. missing scenario deps handled by harness)

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use vectrace::platform::wayland::capture::probe::{run_capture_probe, CapturePathUsed, OverlayHint};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut out_dir: Option<PathBuf> = None;
    let mut scenario_id: Option<String> = None;
    let mut expect_path: Option<String> = None;
    let mut expect_paths: Vec<String> = Vec::new();
    let mut flash_forbidden = false;
    let mut min_w: u32 = 0;
    let mut min_h: u32 = 0;
    let mut expect_overlay: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--scenario" => {
                i += 1;
                scenario_id = args.get(i).cloned();
            }
            "--expect-path" => {
                i += 1;
                if let Some(p) = args.get(i) {
                    expect_path = Some(p.clone());
                    expect_paths.push(p.clone());
                }
            }
            "--expect-path-any" => {
                i += 1;
                if let Some(p) = args.get(i) {
                    for part in p.split('|') {
                        let t = part.trim();
                        if !t.is_empty() {
                            expect_paths.push(t.to_string());
                        }
                    }
                }
            }
            "--flash-forbidden" => flash_forbidden = true,
            "--min-size" => {
                i += 1;
                if let Some(spec) = args.get(i) {
                    let mut parts = spec.split('x');
                    min_w = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    min_h = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                }
            }
            "--expect-overlay" => {
                i += 1;
                expect_overlay = args.get(i).cloned();
            }
            "--help" | "-h" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                print_help();
                return ExitCode::from(1);
            }
        }
        i += 1;
    }

    let out_dir = out_dir.unwrap_or_else(|| {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let sid = scenario_id.as_deref().unwrap_or("adhoc");
        PathBuf::from(format!("e2e/reports/{}/{}", sid, ts))
    });

    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("Failed to create out dir {}: {}", out_dir.display(), e);
        return ExitCode::from(1);
    }

    let png_path = out_dir.join("capture.png");
    let log_path = out_dir.join("stdout.log");

    // Tee-ish: run probe (prints to stdout) and also write a log copy of key fields later.
    let result = run_capture_probe(Some(&png_path));

    let path_str = result
        .path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "failed".to_string());

    let mut failures: Vec<String> = Vec::new();
    if let Some(err) = &result.error {
        if result.path.is_none() {
            failures.push(err.clone());
        } else {
            // PNG write warning only
            eprintln!("warning: {}", err);
        }
    }

    if flash_forbidden {
        if result.path == Some(CapturePathUsed::ScreenshotFlash) {
            failures.push("flash path used but flash_forbidden=true".into());
        }
    }

    if !expect_paths.is_empty() {
        let ok = result
            .path
            .map(|p| expect_paths.iter().any(|e| e == p.as_str()))
            .unwrap_or(false);
        if !ok {
            failures.push(format!(
                "capture_path={} not in expected {:?}",
                path_str, expect_paths
            ));
        }
    } else if let Some(ref expected) = expect_path {
        if result.path.map(|p| p.as_str()) != Some(expected.as_str()) {
            failures.push(format!(
                "capture_path={} expected={}",
                path_str, expected
            ));
        }
    }

    if min_w > 0 && result.width < min_w {
        failures.push(format!("width {} < min {}", result.width, min_w));
    }
    if min_h > 0 && result.height < min_h {
        failures.push(format!("height {} < min {}", result.height, min_h));
    }

    if let Some(ref overlay_expect) = expect_overlay {
        if !overlay_matches(overlay_expect, result.overlay_hint) {
            failures.push(format!(
                "overlay_hint={} expected={}",
                result.overlay_hint.as_str(),
                overlay_expect
            ));
        }
    }

    if result.path.is_some() {
        if let Err(e) = sanity_check_png(&png_path) {
            failures.push(e);
        }
    }

    let exit_ok = failures.is_empty() && result.path.is_some();
    let report = format_report_json(
        scenario_id.as_deref(),
        &result,
        &path_str,
        &failures,
        exit_ok,
    );

    let report_path = out_dir.join("report.json");
    if let Err(e) = fs::write(&report_path, &report) {
        eprintln!("Failed to write {}: {}", report_path.display(), e);
        return ExitCode::from(1);
    }

    let mut log = String::new();
    log.push_str(&format!("scenario={:?}\n", scenario_id));
    log.push_str(&format!("capture_path={}\n", path_str));
    log.push_str(&format!(
        "size={}x{}\n",
        result.width, result.height
    ));
    log.push_str(&format!("overlay_hint={}\n", result.overlay_hint));
    log.push_str(&format!("out_dir={}\n", out_dir.display()));
    for f in &failures {
        log.push_str(&format!("FAIL: {}\n", f));
    }
    let _ = fs::write(&log_path, log);

    println!("Wrote {}", report_path.display());
    if !failures.is_empty() {
        for f in &failures {
            eprintln!("FAIL: {}", f);
        }
        return ExitCode::from(1);
    }
    if result.path.is_none() {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn overlay_matches(expect: &str, actual: OverlayHint) -> bool {
    match expect {
        "layer_shell_or_xwayland" => {
            matches!(actual, OverlayHint::LayerShell | OverlayHint::XWayland)
        }
        other => other == actual.as_str(),
    }
}

fn sanity_check_png(path: &Path) -> Result<(), String> {
    let pixmap = tiny_skia::Pixmap::load_png(path)
        .map_err(|e| format!("cannot load capture PNG: {}", e))?;
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    if w == 0 || h == 0 {
        return Err("empty capture PNG".into());
    }
    let data = pixmap.data();
    let samples = [
        (0usize, 0usize),
        (w / 2, h / 2),
        (w.saturating_sub(1), h.saturating_sub(1)),
        (w / 4, h / 4),
        (3 * w / 4, 3 * h / 4),
    ];
    let mut all_black = true;
    let mut all_transparent = true;
    for (x, y) in samples {
        let i = (y * w + x) * 4;
        let r = data[i];
        let g = data[i + 1];
        let b = data[i + 2];
        let a = data[i + 3];
        if a > 8 {
            all_transparent = false;
        }
        if r > 8 || g > 8 || b > 8 {
            all_black = false;
        }
    }
    if all_transparent {
        return Err("capture looks fully transparent at sample points".into());
    }
    if all_black {
        return Err("capture looks fully black at sample points".into());
    }
    Ok(())
}

fn format_report_json(
    scenario: Option<&str>,
    result: &vectrace::platform::wayland::capture::probe::CaptureProbeResult,
    path_str: &str,
    failures: &[String],
    ok: bool,
) -> String {
    let session = &result.session;
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let fail_arr = if failures.is_empty() {
        "[]".to_string()
    } else {
        let parts: Vec<String> = failures
            .iter()
            .map(|f| format!("\"{}\"", escape(f)))
            .collect();
        format!("[{}]", parts.join(","))
    };
    let png = result
        .png_path
        .as_ref()
        .map(|p| format!("\"{}\"", escape(&p.display().to_string())))
        .unwrap_or_else(|| "null".into());
    let err = result
        .error
        .as_ref()
        .map(|e| format!("\"{}\"", escape(e)))
        .unwrap_or_else(|| "null".into());
    let scenario_json = scenario
        .map(|s| format!("\"{}\"", escape(s)))
        .unwrap_or_else(|| "null".into());

    format!(
        r#"{{
  "scenario_id": {scenario},
  "ok": {ok},
  "capture_path": "{path}",
  "width": {w},
  "height": {h},
  "overlay_hint": "{overlay}",
  "session": {{
    "wayland_display": {wayland},
    "display": {display},
    "session_type": {session_type}
  }},
  "png_path": {png},
  "error": {err},
  "failures": {failures}
}}
"#,
        scenario = scenario_json,
        ok = if ok { "true" } else { "false" },
        path = escape(path_str),
        w = result.width,
        h = result.height,
        overlay = result.overlay_hint.as_str(),
        wayland = opt_str_json(session.wayland_display.as_deref()),
        display = opt_str_json(session.display.as_deref()),
        session_type = opt_str_json(session.session_type.as_deref()),
        png = png,
        err = err,
        failures = fail_arr,
    )
}

fn opt_str_json(v: Option<&str>) -> String {
    match v {
        Some(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "null".into(),
    }
}

fn print_help() {
    let _ = writeln!(
        std::io::stderr(),
        "Usage: vectrace-e2e-probe [options]\n\
         --out-dir DIR\n\
         --scenario ID\n\
         --expect-path PATH\n\
         --expect-path-any a|b\n\
         --expect-overlay HINT\n\
         --flash-forbidden\n\
         --min-size WxH"
    );
}
