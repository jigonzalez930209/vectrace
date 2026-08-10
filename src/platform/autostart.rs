//! XDG autostart helpers (`~/.config/autostart/*.desktop`).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

const AUTOSTART_DESKTOP_NAME: &str = "com.vectrace.Vectrace.desktop";

fn autostart_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config").join("autostart"))
}

pub fn autostart_desktop_path() -> Option<PathBuf> {
    Some(autostart_dir()?.join(AUTOSTART_DESKTOP_NAME))
}

/// True when the XDG autostart desktop entry is present and not hidden.
pub fn is_enabled() -> bool {
    let Some(path) = autostart_desktop_path() else {
        return false;
    };
    let Ok(contents) = fs::read_to_string(&path) else {
        return false;
    };
    if contents
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case("Hidden=true"))
    {
        return false;
    }
    if contents
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false"))
    {
        return false;
    }
    true
}

fn resolve_exec_path() -> String {
    if let Ok(exe) = std::env::current_exe() {
        let s = exe.to_string_lossy();
        // Quote if the path contains spaces.
        if s.contains(' ') {
            return format!("\"{}\"", s);
        }
        return s.into_owned();
    }
    "vectrace".into()
}

fn desktop_entry_contents(exec: &str) -> String {
    format!(
        "\
[Desktop Entry]
Type=Application
Version=1.0
Name=Vectrace
GenericName=Screen Marker & Annotation Tool
Comment=Start Vectrace Screen Marker automatically on login
Exec={exec} --start-in-tray
Icon=vectrace
Terminal=false
Categories=Graphics;2DGraphics;Utility;
Keywords=screen;marker;draw;annotate;presentation;wayland;x11;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"
    )
}

/// Enable or disable login autostart via `~/.config/autostart`.
pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    let dir = autostart_dir().ok_or_else(|| "HOME is not set".to_string())?;
    let path = dir.join(AUTOSTART_DESKTOP_NAME);

    if enabled {
        fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
        let exec = resolve_exec_path();
        let body = desktop_entry_contents(&exec);
        let mut file = fs::File::create(&path)
            .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
        file.write_all(body.as_bytes())
            .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
        println!("Autostart enabled → {}", path.display());
        Ok(true)
    } else {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Cannot remove {}: {}", path.display(), e))?;
            println!("Autostart disabled (removed {})", path.display());
        }
        Ok(false)
    }
}

/// Toggle autostart; returns the new enabled state.
pub fn toggle() -> Result<bool, String> {
    set_enabled(!is_enabled())
}
