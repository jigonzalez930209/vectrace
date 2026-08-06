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
    Exit,
}

pub struct VectraceTray {
    tx: Sender<TrayEvent>,
}

fn create_vectrace_icon(size: i32) -> ksni::Icon {
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    let center = (size as f32) / 2.0;
    let radius = center - 1.5;
    let radius_sq = radius * radius;

    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32) + 0.5 - center;
            let dy = (y as f32) + 0.5 - center;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq <= radius_sq {
                // Vibrant Cyan/Blue Icon (ARGB format)
                pixels.push(255); // Alpha
                pixels.push(40);  // Red
                pixels.push(130); // Green
                pixels.push(245); // Blue
            } else if dist_sq <= (radius + 1.0) * (radius + 1.0) {
                // Anti-aliased outer edge
                pixels.push(128); // Alpha
                pixels.push(40);  // Red
                pixels.push(130); // Green
                pixels.push(245); // Blue
            } else {
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

        vec![
            StandardItem {
                label: "Vectrace Screen Marker".into(),
                enabled: false,
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "👁️ Show / Hide Overlay".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Show / Hide Overlay clicked.");
                    let _ = tx1.send(TrayEvent::ToggleVisibility);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "⚙️ Settings Menu".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Settings clicked.");
                    let _ = tx2.send(TrayEvent::ToggleSettingsMenu);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "🖥️ Switch Display (Primary / All)".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Switch Display clicked.");
                    let _ = tx3.send(TrayEvent::ToggleMonitorMode);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "🖱️ Toggle Click-Through".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Toggle Click-Through clicked.");
                    let _ = tx4.send(TrayEvent::TogglePassthrough);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "🎨 Cycle Background Mode".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Cycle Background clicked.");
                    let _ = tx5.send(TrayEvent::CycleBackground);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "🧹 Clear Canvas".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Clear Canvas clicked.");
                    let _ = tx6.send(TrayEvent::ClearCanvas);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Global Shortcut: [Ctrl + Alt + A]".into(),
                enabled: false,
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "❌ Quit Vectrace".into(),
                activate: Box::new(move |_| {
                    println!("System Tray DBus Menu: Quit Vectrace clicked.");
                    let _ = tx7.send(TrayEvent::Exit);
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
        println!("System Tray Icon clicked directly (activate).");
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
    println!("System Tray Icon (StatusNotifierItem - Active) initialized with interactive menu.");
}



