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
    let margin = (fsize * 0.1).round();
    let inner_w = fsize - margin * 2.0;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32;
            let fy = y as f32;

            // Lucide-style Minimalist Vector Pen & Frame Outline (Monochrome White on Dark Badge)
            let is_in_badge = fx >= margin && fx < fsize - margin && fy >= margin && fy < fsize - margin;
            
            // Clean Lucide diagonal pen stroke & crop box corner lines
            let is_pen_line = (fx - fy).abs() <= (fsize * 0.08) && fx >= margin * 1.5 && fx <= fsize - margin * 1.5;
            let is_corner_h = (fy == margin || fy == fsize - margin - 1.0) && fx >= margin && fx <= margin + inner_w * 0.35;
            let is_corner_v = (fx == margin || fx == fsize - margin - 1.0) && fy >= margin && fy <= margin + inner_w * 0.35;

            if is_pen_line || is_corner_h || is_corner_v {
                // High-contrast Lucide White (ARGB)
                pixels.push(255); // Alpha
                pixels.push(250); // Red
                pixels.push(250); // Green
                pixels.push(250); // Blue
            } else if is_in_badge {
                // Sleek Dark Charcoal Glass Badge
                pixels.push(220); // Alpha
                pixels.push(20);  // Red
                pixels.push(22);  // Green
                pixels.push(28);  // Blue
            } else {
                // Transparent outer border
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
        "edit-select".to_string()
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
