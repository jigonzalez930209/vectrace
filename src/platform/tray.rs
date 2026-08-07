use ksni::{Tray, TrayMethods, MenuItem};
use ksni::menu::StandardItem;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayEvent {
    ToggleVisibility,
    ToggleSettingsMenu,
    ToggleMonitorMode,
    TogglePassthrough,
    CycleBackground,
    ClearCanvas,
    SaveFull,
    SaveRegion,
    Exit,
}

pub struct VectraceTray {
    tx: Sender<TrayEvent>,
}

fn create_vectrace_icon(size: i32) -> ksni::Icon {
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    let fsize = size as f32;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32;
            let fy = y as f32;

            // Normalize coordinates to [-1.0, 1.0]
            let nx = (fx - fsize / 2.0) / (fsize / 2.0);
            let ny = (fy - fsize / 2.0) / (fsize / 2.0);
            let dist_center = (nx * nx + ny * ny).sqrt();

            // Outer circular badge (#141722)
            let in_badge = dist_center <= 0.88;

            // Diagonal Vector Pen / Stylus line (from top-right to bottom-left)
            // Line: nx + ny = 0  => dist = |nx + ny| / sqrt(2)
            let line_dist = (nx + ny).abs() / 1.4142;
            let in_stem = line_dist <= 0.18 && (nx - ny).abs() <= 0.70;

            // Cyan glowing nib at tip (bottom-left)
            let nib_dist = ((nx + 0.45).powi(2) + (ny - 0.45).powi(2)).sqrt();
            let in_nib = nib_dist <= 0.22;

            // Top-right handle accent
            let top_dist = ((nx - 0.45).powi(2) + (ny + 0.45).powi(2)).sqrt();
            let in_top = top_dist <= 0.22;

            if in_nib {
                // Bright Electric Cyan Nib (#00f0ff) in ARGB (A, R, G, B)
                pixels.push(255); // Alpha
                pixels.push(0);   // Red
                pixels.push(240); // Green
                pixels.push(255); // Blue
            } else if in_stem || in_top {
                // Pure White Stylus Body (#ffffff)
                pixels.push(255); // Alpha
                pixels.push(245); // Red
                pixels.push(248); // Green
                pixels.push(250); // Blue
            } else if in_badge {
                // Deep Charcoal Glass Badge (#161a24)
                pixels.push(245); // Alpha
                pixels.push(22);  // Red
                pixels.push(26);  // Green
                pixels.push(36);  // Blue
            } else {
                // Transparent border
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
                pixels.push(0);
            }
        }
    }
    ksni::Icon {
        width: size,
        height: size,
        data: pixels,
    }
}

impl Tray for VectraceTray {
    fn id(&self) -> String {
        "vectrace-screen-marker".to_string()
    }

    fn title(&self) -> String {
        "Vectrace Screen Marker".to_string()
    }

    fn icon_name(&self) -> String {
        // Return empty string so StatusNotifierHost falls back to custom ARGB icon_pixmap
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![
            create_vectrace_icon(16),
            create_vectrace_icon(22),
            create_vectrace_icon(24),
            create_vectrace_icon(32),
            create_vectrace_icon(48),
            create_vectrace_icon(64),
        ]
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx1 = self.tx.clone();
        let tx2 = self.tx.clone();
        let tx3 = self.tx.clone();
        let tx4 = self.tx.clone();
        let tx5 = self.tx.clone();
        let tx6 = self.tx.clone();
        let tx7 = self.tx.clone();
        let tx8 = self.tx.clone();
        let tx9 = self.tx.clone();

        vec![
            StandardItem {
                label: "Vectrace Screen Marker".into(),
                enabled: false,
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Show / Hide Overlay".into(),
                activate: Box::new(move |_| {
                    let _ = tx1.send(TrayEvent::ToggleVisibility);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Save Full Screen".into(),
                activate: Box::new(move |_| {
                    let _ = tx7.send(TrayEvent::SaveFull);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "Save Region Selection (Crop)".into(),
                activate: Box::new(move |_| {
                    let _ = tx8.send(TrayEvent::SaveRegion);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Toggle Click-Through".into(),
                activate: Box::new(move |_| {
                    let _ = tx4.send(TrayEvent::TogglePassthrough);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "Cycle Background Mode".into(),
                activate: Box::new(move |_| {
                    let _ = tx5.send(TrayEvent::CycleBackground);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "Clear Canvas".into(),
                activate: Box::new(move |_| {
                    let _ = tx6.send(TrayEvent::ClearCanvas);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Switch Display Mode".into(),
                activate: Box::new(move |_| {
                    let _ = tx3.send(TrayEvent::ToggleMonitorMode);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "Settings Menu".into(),
                activate: Box::new(move |_| {
                    let _ = tx2.send(TrayEvent::ToggleSettingsMenu);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit Vectrace".into(),
                activate: Box::new(move |_| {
                    let _ = tx9.send(TrayEvent::Exit);
                }),
                ..Default::default()
            }.into(),
        ]
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayEvent::ToggleVisibility);
    }
}

pub fn spawn_tray(tx: Sender<TrayEvent>) {
    let tray = VectraceTray { tx };
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            if let Ok(handle) = tray.spawn().await {
                Box::leak(Box::new(handle));
                std::future::pending::<()>().await;
            }
        });
    });
    println!("System Tray Icon (StatusNotifierItem - Lucide Style Monochrome) initialized.");
}
