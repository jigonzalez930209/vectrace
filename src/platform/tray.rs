use ksni::{Tray, TrayMethods, MenuItem};
use ksni::menu::StandardItem;
use std::sync::{mpsc::Sender, OnceLock};

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

const LOGO_SVG: &[u8] = include_bytes!("../../assets/vectrace.svg");

fn logo_tree() -> &'static resvg::usvg::Tree {
    static TREE: OnceLock<resvg::usvg::Tree> = OnceLock::new();
    TREE.get_or_init(|| {
        let opt = resvg::usvg::Options::default();
        resvg::usvg::Tree::from_data(LOGO_SVG, &opt).expect("assets/vectrace.svg must parse")
    })
}

/// Rasterize the brand SVG into a StatusNotifierItem ARGB pixmap.
fn create_vectrace_icon(size: i32) -> ksni::Icon {
    let size_u = size.max(1) as u32;
    let tree = logo_tree();
    let mut pixmap = tiny_skia::Pixmap::new(size_u, size_u).expect("tray pixmap");

    let svg = tree.size();
    let scale = (size as f32) / svg.width().max(svg.height()).max(1.0);
    let tx = (size as f32 - svg.width() * scale) * 0.5;
    let ty = (size as f32 - svg.height() * scale) * 0.5;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);

    resvg::render(tree, transform, &mut pixmap.as_mut());

    // Freedesktop IconPixmap: ARGB32 network byte order (A, R, G, B).
    let mut data = Vec::with_capacity((size_u * size_u * 4) as usize);
    for px in pixmap.pixels() {
        data.push(px.alpha());
        data.push(px.red());
        data.push(px.green());
        data.push(px.blue());
    }

    ksni::Icon {
        width: size,
        height: size,
        data,
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
        // Empty → StatusNotifierHost uses icon_pixmap (embedded brand SVG).
        // Installed packages still ship assets/vectrace.svg as the hicolor app icon.
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
    println!("System Tray Icon (brand logo from assets/vectrace.svg) initialized.");
}
