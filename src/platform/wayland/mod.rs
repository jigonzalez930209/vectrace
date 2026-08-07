pub mod capture;

use crate::core::{Canvas, Point, Tool, StrokeType, MonitorMode};
use crate::platform::PlatformBackend;
use crate::ui::{Toolbar, ToolbarAction};
use std::error::Error;
use std::os::fd::AsFd;


use wayland_client::{
    delegate_noop,
    globals::GlobalListContents,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};
use wayland_protocols::xdg::shell::client::{
    xdg_wm_base::{self, XdgWmBase},
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
};

pub struct WaylandState {
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub layer_shell: Option<ZwlrLayerShellV1>,
    pub xdg_wm_base: Option<XdgWmBase>,
    pub seat: Option<wl_seat::WlSeat>,
    pub surface: Option<wl_surface::WlSurface>,
    pub layer_surface: Option<ZwlrLayerSurfaceV1>,
    pub xdg_surface: Option<XdgSurface>,
    pub xdg_toplevel: Option<XdgToplevel>,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    /// True when using zwlr layer-shell (Exclusive keyboard available).
    pub uses_layer_shell: bool,
    pub should_exit: bool,
    
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    
    pub pointer_x: f64,
    pub pointer_y: f64,
    pub button_pressed: bool,
    pub last_button_time: u32,
    
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,
    pub pending_key: Option<(u32, u32)>, // keycode, state
}

impl WaylandState {
    pub fn new() -> Self {
        Self {
            compositor: None,
            shm: None,
            layer_shell: None,
            xdg_wm_base: None,
            seat: None,
            surface: None,
            layer_surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            pointer: None,
            keyboard: None,
            uses_layer_shell: false,
            should_exit: false,
            configured: false,
            width: 0,
            height: 0,
            pointer_x: 0.0,
            pointer_y: 0.0,
            button_pressed: false,
            last_button_time: 0,
            shift_pressed: false,
            ctrl_pressed: false,
            pending_key: None,
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, height } = event {
            layer_surface.ack_configure(serial);
            if width > 0 && height > 0 {
                state.width = width;
                state.height = height;
            }
            state.configured = true;
        }
    }
}

impl Dispatch<XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, ()> for WaylandState {
    fn event(
        state: &mut Self,
        xdg_surface: &XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<XdgToplevel, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0 && height > 0 {
                    state.width = width as u32;
                    state.height = height as u32;
                }
            }
            xdg_toplevel::Event::Close => {
                state.should_exit = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                state.pointer_x = surface_x;
                state.pointer_y = surface_y;
            }
            wl_pointer::Event::Button { time, button, state: btn_state, .. } => {
                if button == 272 {
                    state.button_pressed = match btn_state {
                        wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => true,
                        _ => false,
                    };
                    state.last_button_time = time;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key { key, state: key_state, .. } = event {
            let is_pressed = match key_state {
                wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed) => 1,
                _ => 0,
            };
            if key == 42 || key == 54 {
                state.shift_pressed = is_pressed == 1;
            }
            // Left Ctrl (29) / Right Ctrl (97) — Linux evdev codes
            if key == 29 || key == 97 {
                state.ctrl_pressed = is_pressed == 1;
            }
            state.pending_key = Some((key, is_pressed));
        }
    }
}

delegate_noop!(WaylandState: ignore wl_compositor::WlCompositor);
delegate_noop!(WaylandState: ignore wl_shm::WlShm);
delegate_noop!(WaylandState: ignore ZwlrLayerShellV1);
delegate_noop!(WaylandState: ignore wl_seat::WlSeat);
delegate_noop!(WaylandState: ignore wl_surface::WlSurface);
delegate_noop!(WaylandState: ignore wl_buffer::WlBuffer);
delegate_noop!(WaylandState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(WaylandState: ignore wl_region::WlRegion);

fn create_shm_file(size: usize) -> std::io::Result<std::fs::File> {
    use rustix::fs::MemfdFlags;
    let name = c"vectrace-shm";
    let fd = rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let file = std::fs::File::from(fd);
    file.set_len(size as u64)?;
    Ok(file)
}

fn detect_scale_factor() -> f32 {
    if let Ok(val) = std::env::var("GDK_SCALE") {
        if let Ok(scale) = val.parse::<f32>() {
            return scale.max(1.0);
        }
    }
    if let Ok(val) = std::env::var("QT_SCALE_FACTOR") {
        if let Ok(scale) = val.parse::<f32>() {
            return scale.max(1.0);
        }
    }
    if let Ok(val) = std::env::var("VECTRACE_SCALE") {
        if let Ok(scale) = val.parse::<f32>() {
            return scale.max(1.0);
        }
    }
    1.0
}

fn evdev_key_to_char(key: u32, shift: bool) -> Option<char> {
    match (key, shift) {
        (30, false) => Some('a'), (30, true) => Some('A'),
        (48, false) => Some('b'), (48, true) => Some('B'),
        (46, false) => Some('c'), (46, true) => Some('C'),
        (32, false) => Some('d'), (32, true) => Some('D'),
        (18, false) => Some('e'), (18, true) => Some('E'),
        (33, false) => Some('f'), (33, true) => Some('F'),
        (34, false) => Some('g'), (34, true) => Some('G'),
        (35, false) => Some('h'), (35, true) => Some('H'),
        (23, false) => Some('i'), (23, true) => Some('I'),
        (36, false) => Some('j'), (36, true) => Some('J'),
        (37, false) => Some('k'), (37, true) => Some('K'),
        (38, false) => Some('l'), (38, true) => Some('L'),
        (50, false) => Some('m'), (50, true) => Some('M'),
        (49, false) => Some('n'), (49, true) => Some('N'),
        (24, false) => Some('o'), (24, true) => Some('O'),
        (25, false) => Some('p'), (25, true) => Some('P'),
        (16, false) => Some('q'), (16, true) => Some('Q'),
        (19, false) => Some('r'), (19, true) => Some('R'),
        (31, false) => Some('s'), (31, true) => Some('S'),
        (20, false) => Some('t'), (20, true) => Some('T'),
        (22, false) => Some('u'), (22, true) => Some('U'),
        (47, false) => Some('v'), (47, true) => Some('V'),
        (17, false) => Some('w'), (17, true) => Some('W'),
        (45, false) => Some('x'), (45, true) => Some('X'),
        (21, false) => Some('y'), (21, true) => Some('Y'),
        (44, false) => Some('z'), (44, true) => Some('Z'),
        (57, _) => Some(' '),
        (11, false) => Some('0'), (11, true) => Some(')'),
        (2, false) => Some('1'), (2, true) => Some('!'),
        (3, false) => Some('2'), (3, true) => Some('@'),
        (4, false) => Some('3'), (4, true) => Some('#'),
        (5, false) => Some('4'), (5, true) => Some('$'),
        (6, false) => Some('5'), (6, true) => Some('%'),
        (7, false) => Some('6'), (7, true) => Some('^'),
        (8, false) => Some('7'), (8, true) => Some('&'),
        (9, false) => Some('8'), (9, true) => Some('*'),
        (10, false) => Some('9'), (10, true) => Some('('),
        _ => None,
    }
}

use crate::platform::tray::TrayEvent;
use std::sync::mpsc::Receiver;

pub struct WaylandBackend {
    passthrough: bool,
    active_tool: Tool,
    scale_factor: f32,
    show_settings_menu: bool,
    show_color_menu: bool,
    monitor_mode: MonitorMode,
    is_hidden: bool,
    tray_rx: Option<Receiver<TrayEvent>>,
}

impl WaylandBackend {
    pub fn new() -> Self {
        let scale_factor = detect_scale_factor();
        Self {
            passthrough: false,
            active_tool: Tool::default_pen(),
            scale_factor,
            show_settings_menu: false,
            show_color_menu: false,
            monitor_mode: MonitorMode::Primary,
            is_hidden: false,
            tray_rx: None,
        }
    }



    pub fn new_with_tray(tray_rx: Receiver<TrayEvent>) -> Self {
        let mut backend = Self::new();
        backend.tray_rx = Some(tray_rx);
        backend
    }
}



impl PlatformBackend for WaylandBackend {
    fn run(&mut self, canvas: &mut Canvas) -> Result<(), Box<dyn Error>> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut event_queue) = wayland_client::globals::registry_queue_init::<WaylandState>(&conn)?;
        let qh = event_queue.handle();

        let mut state = WaylandState::new();

        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=5, ())?;
        let shm = globals.bind::<wl_shm::WlShm, _, _>(&qh, 1..=1, ())?;
        
        if let Ok(seat) = globals.bind::<wl_seat::WlSeat, _, _>(&qh, 1..=7, ()) {
            state.pointer = Some(seat.get_pointer(&qh, ()));
            state.keyboard = Some(seat.get_keyboard(&qh, ()));
            state.seat = Some(seat);
        }

        state.compositor = Some(compositor.clone());
        state.shm = Some(shm.clone());

        let surface = compositor.create_surface(&qh, ());
        state.surface = Some(surface.clone());

        if let Ok(layer_shell) = globals.bind::<ZwlrLayerShellV1, _, _>(&qh, 1..=4, ()) {
            println!("Using native Wayland Layer-Shell protocol...");
            let layer_surface = layer_shell.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Overlay,
                "vectrace".to_string(),
                &qh,
                (),
            );
            layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
            layer_surface.set_exclusive_zone(-1);
            // Exclusive: tool shortcuts work reliably. When click-through is on we
            // switch to None so keystrokes reach the application underneath.
            layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            state.layer_shell = Some(layer_shell);
            state.layer_surface = Some(layer_surface);
            state.uses_layer_shell = true;
        } else {
            // GNOME and similar: no wlr-layer-shell. Fullscreen xdg-shell windows are
            // intentionally opaque on Mutter (Wayland protocol / compositor policy),
            // so a see-through annotation overlay must use XWayland 32-bit ARGB.
            println!("Layer-Shell not available (e.g. GNOME Wayland).");
            println!("Note: GNOME blocks transparency for fullscreen xdg-shell; using XWayland ARGB overlay...");
            let mut x11 = if let Some(rx) = self.tray_rx.take() {
                crate::platform::x11::X11Backend::new_with_tray(rx)
            } else {
                crate::platform::x11::X11Backend::new()
            };
            return x11.run(canvas);
        }



        surface.commit();
        event_queue.roundtrip(&mut state)?;

        while !state.configured {
            event_queue.blocking_dispatch(&mut state)?;
        }

        let width = if state.width > 0 { state.width } else { 1920 };
        let height = if state.height > 0 { state.height } else { 1080 };

        canvas.resize(width, height);
        canvas.set_scale_factor(self.scale_factor);
        println!("Wayland Layer-Shell overlay initialized: {}x{} (Scale: {:.1}x)", width, height, self.scale_factor);
        println!("Controls: [Space] Click-Through  [P/H/L/A/R/O/K/N/E/T] Tools  [U] Undo  [Ctrl+R] Redo  [C] Clear  [ESC] Exit");

        let toolbar = Toolbar::new_with_scale(width as f32, self.scale_factor);

        let stride = (width * 4) as usize;
        let buffer_size = stride * height as usize;
        let shm_file = create_shm_file(buffer_size)?;
        
        let pool = shm.create_pool(shm_file.as_fd(), buffer_size as i32, &qh, ());
        let wl_buf = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );

        let mmap = unsafe {
            rustix::mm::mmap(
                std::ptr::null_mut(),
                buffer_size,
                rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE,
                rustix::mm::MapFlags::SHARED,
                &shm_file,
                0,
            )?
        };

        let mut base_pixmap = tiny_skia::Pixmap::new(width, height).unwrap();

        let mut prev_button_pressed = false;

        apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);

        loop {
            if state.should_exit {
                println!("xdg-shell window closed.");
                break;
            }

            // Process any incoming TrayEvent messages from system tray
            if let Some(ref rx) = self.tray_rx {
                while let Ok(tray_event) = rx.try_recv() {
                    match tray_event {
                        TrayEvent::ToggleVisibility => {
                            self.is_hidden = !self.is_hidden;
                            self.passthrough = self.is_hidden;
                            println!("System Tray Action: Toggle Visibility (is_hidden = {})", self.is_hidden);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }

                        TrayEvent::ToggleSettingsMenu => {
                            self.show_settings_menu = !self.show_settings_menu;
                            println!("System Tray Action: Toggle Settings Menu = {}", self.show_settings_menu);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }
                        TrayEvent::ToggleMonitorMode => {
                            self.monitor_mode = self.monitor_mode.toggle();
                            println!("System Tray Action: Toggle Monitor Mode = {:?}", self.monitor_mode);
                        }
                        TrayEvent::TogglePassthrough => {
                            self.passthrough = !self.passthrough;
                            println!("System Tray Action: Toggle Passthrough = {}", self.passthrough);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }

                        TrayEvent::CycleBackground => {
                            let mode = canvas.cycle_background_mode();
                            println!("System Tray Action: Cycle Background = {:?}", mode);
                        }
                        TrayEvent::ClearCanvas => {
                            canvas.clear();
                            println!("System Tray Action: Clear Canvas");
                        }
                        TrayEvent::SaveFull => {
                            trigger_wayland_save_full(canvas, state.width as u32, state.height as u32, &surface, &wl_buf, &mut event_queue, &mut state);
                        }
                        TrayEvent::SaveRegion => {
                            self.active_tool = Tool::default_select_region();
                            println!("System Tray Action: Select Region Crop Tool");
                        }
                        TrayEvent::Exit => {
                            println!("System Tray Action: Exit application");
                            return Ok(());
                        }
                    }
                }
            }

            event_queue.dispatch_pending(&mut state)?;


            let cur_x = state.pointer_x as f32;
            let cur_y = state.pointer_y as f32;
            let now_ms = crate::core::canvas::current_time_ms();

            if state.button_pressed && !prev_button_pressed {
                let has_crop = false;
                if let Some(action) = toolbar.handle_click(cur_x, cur_y, self.show_settings_menu, self.show_color_menu, has_crop) {
                    if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                        canvas.finish_current_stroke();
                    }

                    match action {
                        ToolbarAction::StartDrag => {}
                        ToolbarAction::ConfirmCrop => {}
                        ToolbarAction::SelectTool(tool) => {
                            self.active_tool = tool;
                            self.show_color_menu = false;
                            println!("Selected tool: {:?}", tool);
                        }
                        ToolbarAction::SelectShape(kind) => {
                            self.active_tool = Tool::default_shape(kind);
                            self.show_color_menu = false;
                            println!("Selected shape: {:?}", kind);
                        }
                        ToolbarAction::SetColor(color) => {
                            self.active_tool.set_color(color);
                            self.show_color_menu = false;
                            println!("Set color: {:?}", color);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }
                        ToolbarAction::ToggleColorMenu => {
                            self.show_color_menu = !self.show_color_menu;
                            self.show_settings_menu = false;
                            println!("Toggled Color Menu: {}", self.show_color_menu);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }
                        ToolbarAction::ToggleBackgroundMode => {
                            let mode = canvas.cycle_background_mode();
                            println!("Switched background mode to: {:?}", mode);
                        }
                        ToolbarAction::Clear => {
                            canvas.clear();
                            println!("Canvas cleared");
                        }
                        ToolbarAction::SaveFull => {
                            trigger_wayland_save_full(canvas, state.width as u32, state.height as u32, &surface, &wl_buf, &mut event_queue, &mut state);
                        }
                        ToolbarAction::TogglePassthrough => {
                            self.passthrough = !self.passthrough;
                            println!("Toggled Click-Through: {}", self.passthrough);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }
                        ToolbarAction::ToggleSettingsMenu => {
                            self.show_settings_menu = !self.show_settings_menu;
                            println!("Toggled Settings Menu: {}", self.show_settings_menu);
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }
                        ToolbarAction::ToggleMonitorMode => {
                            self.monitor_mode = self.monitor_mode.toggle();
                            println!("Switched Monitor Mode: {:?}", self.monitor_mode);
                        }
                        ToolbarAction::MinimizeToTray => {
                            self.is_hidden = true;
                            self.passthrough = true;
                            println!("Minimized overlay to System Tray.");
                            apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                        }


                        ToolbarAction::Exit => {
                            println!("Exiting via toolbar...");
                            break;
                        }
                    }
                } else if !self.passthrough {

                    if matches!(self.active_tool, Tool::Text { .. }) {
                        if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                            canvas.finish_current_stroke();
                        }
                        if let Some(mut stroke) = self.active_tool.create_stroke() {
                            stroke.points = vec![Point::new(cur_x, cur_y, 1.0, now_ms)];
                            canvas.start_stroke(stroke);
                            println!("Started Text stroke on Wayland at ({:.0}, {:.0})", cur_x, cur_y);
                        }
                    } else {
                        if let Some(stroke) = self.active_tool.create_stroke() {
                            canvas.start_stroke(stroke);
                            canvas.add_point_to_current_stroke(Point::new(cur_x, cur_y, 1.0, now_ms));
                        }
                    }
                }
            } else if state.button_pressed && prev_button_pressed {
                if canvas.current_stroke().is_some() && !self.passthrough && !matches!(self.active_tool, Tool::Text { .. }) {
                    let is_shape = canvas.current_stroke().map_or(false, |s| s.stroke_type != StrokeType::Freehand && s.stroke_type != StrokeType::Text && s.stroke_type != StrokeType::Laser && s.stroke_type != StrokeType::Spotlight);
                    let is_spotlight = canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Spotlight);

                    if is_spotlight {
                        if let Some(stroke) = canvas.current_stroke_mut() {
                            stroke.points = vec![Point::new(cur_x, cur_y, 1.0, now_ms)];
                        }
                    } else if is_shape {
                        if let Some(stroke) = canvas.current_stroke_mut() {
                            if stroke.points.len() >= 2 {
                                stroke.points[1] = Point::new(cur_x, cur_y, 1.0, now_ms);
                            } else {
                                stroke.add_point(Point::new(cur_x, cur_y, 1.0, now_ms));
                            }
                        }
                    } else {
                        canvas.add_point_to_current_stroke(Point::new(cur_x, cur_y, 1.0, now_ms));
                    }
                }
            } else if !state.button_pressed && prev_button_pressed {
                if canvas.current_stroke().is_some() && !matches!(self.active_tool, Tool::Text { .. }) {
                    canvas.finish_current_stroke();
                }
            }

            prev_button_pressed = state.button_pressed;

            if let Some((key_code, is_pressed)) = state.pending_key.take() {
                if is_pressed == 1 {
                    // Click-through: keyboard interactivity is None, but ignore
                    // shortcuts if any key still arrives so the app below can type.
                    if self.passthrough {
                        // allow nothing — Space toggle is via toolbar while click-through
                    } else {
                    let is_typing_text = matches!(self.active_tool, Tool::Text { .. });

                    if is_typing_text {
                        if canvas.current_stroke().is_none() {
                            if let Some(mut stroke) = self.active_tool.create_stroke() {
                                stroke.points = vec![Point::new(
                                    state.width as f32 / 2.0,
                                    state.height as f32 / 2.0,
                                    1.0,
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as u64,
                                )];
                                canvas.start_stroke(stroke);
                            }
                        }
                        match key_code {
                            28 => { // Enter
                                canvas.finish_current_stroke();
                                println!("Committed text on Wayland");
                            }
                            14 => { // Backspace
                                if let Some(stroke) = canvas.current_stroke_mut() {
                                    if let Some(ref mut text) = stroke.text_content {
                                        text.pop();
                                    }
                                }
                            }
                            1 => { // Escape
                                canvas.cancel_current_stroke();
                                println!("Cancelled text on Wayland");
                            }
                            _ => {
                                if let Some(ch) = evdev_key_to_char(key_code, state.shift_pressed) {
                                    if let Some(stroke) = canvas.current_stroke_mut() {
                                        let text = stroke.text_content.get_or_insert_with(String::new);
                                        text.push(ch);
                                        println!("Typed char on Wayland: {:?}", ch);
                                    }
                                }
                            }
                        }
                    } else {
                        match key_code {
                            1 => { // Escape
                                println!("Exiting...");
                                break;
                            }
                            25 => { // P - Pencil
                                let mut tool = Tool::default_pen();
                                if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                self.active_tool = tool;
                                println!("Tool: Pencil");
                            }
                            35 => { // H - Highlighter
                                let mut tool = Tool::default_highlighter();
                                if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                self.active_tool = tool;
                                println!("Tool: Highlighter");
                            }
                            38 => { // L - Line
                                let mut tool = Tool::default_shape(crate::core::ShapeKind::Line);
                                if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                self.active_tool = tool;
                                println!("Tool: Line");
                            }
                            30 => { // A - Arrow
                                let mut tool = Tool::default_shape(crate::core::ShapeKind::Arrow);
                                if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                self.active_tool = tool;
                                println!("Tool: Arrow");
                            }
                            19 => { // R - Rectangle / Ctrl+R - Redo
                                if state.ctrl_pressed {
                                    if canvas.redo() {
                                        println!("Redo stroke");
                                    }
                                } else {
                                    let mut tool = Tool::default_shape(crate::core::ShapeKind::Rectangle);
                                    if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                    self.active_tool = tool;
                                    println!("Tool: Rectangle");
                                }
                            }
                            24 => { // O - Oval
                                let mut tool = Tool::default_shape(crate::core::ShapeKind::Oval);
                                if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                self.active_tool = tool;
                                println!("Tool: Oval");
                            }
                            37 => { // K - Laser
                                self.active_tool = Tool::default_laser();
                                println!("Tool: Laser");
                            }
                            49 => { // N - Spotlight
                                self.active_tool = Tool::default_spotlight();
                                println!("Tool: Spotlight");
                            }
                            18 => { // E - Eraser
                                self.active_tool = Tool::default_eraser();
                                println!("Tool: Eraser");
                            }
                            20 => { // T - Text
                                let mut tool = Tool::default_text();
                                if let Some(c) = self.active_tool.color() { tool.set_color(c); }
                                self.active_tool = tool;
                                println!("Tool: Text");
                            }
                            31 => { // S - Save Full
                                trigger_wayland_save_full(canvas, state.width as u32, state.height as u32, &surface, &wl_buf, &mut event_queue, &mut state);
                            }
                            50 => { // M - Minimize
                                self.is_hidden = true;
                                self.passthrough = true;
                                apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                            }
                            57 => { // Space
                                self.passthrough = !self.passthrough;
                                println!("Toggled Click-Through: {}", self.passthrough);
                                apply_wayland_passthrough(&compositor, &surface, state.layer_surface.as_ref(), state.xdg_toplevel.as_ref(), self.passthrough, self.show_settings_menu, self.show_color_menu, &toolbar, &qh);
                            }
                            48 => { // B
                                let mode = canvas.cycle_background_mode();
                                println!("Switched background mode to: {:?}", mode);
                            }
                            22 => { // U
                                if canvas.undo() {
                                    println!("Undo stroke");
                                }
                            }
                            46 => { // C
                                canvas.clear();
                                println!("Canvas cleared");
                            }
                            _ => {}
                        }
                    }
                    } // end !passthrough
                }
            }

            canvas.render_background(&mut base_pixmap);
            canvas.render_completed_strokes(&mut base_pixmap);
            canvas.render_current_stroke(&mut base_pixmap);
            toolbar.draw(
                &mut base_pixmap,
                self.active_tool,
                self.passthrough,
                canvas.background_mode,
                self.show_settings_menu,
                self.show_color_menu,
                self.monitor_mode,
                false,
                None,
            );


            let shm_slice = unsafe {
                std::slice::from_raw_parts_mut(mmap as *mut u8, buffer_size)
            };
            let src = base_pixmap.data();
            let pixel_count = (width * height) as usize;
            for p in 0..pixel_count {
                let s = p * 4;
                shm_slice[s] = src[s + 2];     // B
                shm_slice[s + 1] = src[s + 1]; // G
                shm_slice[s + 2] = src[s];     // R
                shm_slice[s + 3] = src[s + 3]; // A
            }


            surface.attach(Some(&wl_buf), 0, 0);
            surface.damage_buffer(0, 0, width as i32, height as i32);
            surface.commit();

            conn.flush()?;
            event_queue.blocking_dispatch(&mut state)?;
        }

        unsafe {
            let _ = rustix::mm::munmap(mmap, buffer_size);
        }

        Ok(())
    }
}

fn apply_wayland_passthrough(
    compositor: &wl_compositor::WlCompositor,
    surface: &wl_surface::WlSurface,
    layer_surface: Option<&ZwlrLayerSurfaceV1>,
    _xdg_toplevel: Option<&XdgToplevel>,
    passthrough: bool,
    show_settings_menu: bool,
    show_color_menu: bool,
    toolbar: &Toolbar,
    qh: &QueueHandle<WaylandState>,
) {
    if passthrough {
        // Pointer hits only the toolbar/menus so clicks reach apps underneath.
        let region = compositor.create_region(qh, ());
        region.add(
            toolbar.x as i32,
            toolbar.y as i32,
            toolbar.width as i32,
            toolbar.height as i32,
        );
        if show_color_menu {
            let menu_x = toolbar.x + toolbar.color_btn_logical_x() * toolbar.scale_factor;
            let menu_y = toolbar.y + toolbar.height + 6.0 * toolbar.scale_factor;
            let menu_w = 150.0 * toolbar.scale_factor;
            let menu_h = 110.0 * toolbar.scale_factor;
            region.add(menu_x as i32, menu_y as i32, menu_w as i32, menu_h as i32);
        }
        if show_settings_menu {
            let menu_x = toolbar.x + toolbar.settings_btn_logical_x() * toolbar.scale_factor;
            let menu_y = toolbar.y + toolbar.height + 6.0 * toolbar.scale_factor;
            let menu_w = 240.0 * toolbar.scale_factor;
            let menu_h = 130.0 * toolbar.scale_factor;
            region.add(menu_x as i32, menu_y as i32, menu_w as i32, menu_h as i32);
        }
        surface.set_input_region(Some(&region));
        region.destroy();
        if let Some(ls) = layer_surface {
            ls.set_keyboard_interactivity(KeyboardInteractivity::None);
        }
    } else {
        // Full overlay captures pointer; layer-shell can take Exclusive keyboard.
        surface.set_input_region(None);
        if let Some(ls) = layer_surface {
            ls.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        }
    }
    surface.commit();
}

fn trigger_wayland_save_full(
    canvas: &mut Canvas,
    width: u32,
    height: u32,
    surface: &wl_surface::WlSurface,
    wl_buf: &wl_buffer::WlBuffer,
    event_queue: &mut wayland_client::EventQueue<WaylandState>,
    state: &mut WaylandState,
) {
    if width == 0 || height == 0 { return; }

    let bg_mode = canvas.background_mode;
    let doc = canvas.snapshot();

    surface.attach(None, 0, 0);
    surface.commit();
    let _ = event_queue.roundtrip(state);
    std::thread::sleep(std::time::Duration::from_millis(50));

    surface.attach(Some(wl_buf), 0, 0);
    surface.damage(0, 0, width as i32, height as i32);
    surface.commit();
    let _ = event_queue.roundtrip(state);

    std::thread::spawn(move || {
        let captured = if bg_mode == crate::core::BackgroundMode::Transparent {
            Some(crate::platform::wayland::capture::portal::PortalClient::take_screenshot())
        } else {
            None
        };

        let mut temp_pixmap = match captured {
            Some(Ok(desktop_pixmap)) => {
                println!(
                    "Captured desktop background ({}x{})!",
                    desktop_pixmap.width(),
                    desktop_pixmap.height()
                );
                if desktop_pixmap.width() == width && desktop_pixmap.height() == height {
                    desktop_pixmap
                } else {
                    let mut scaled = tiny_skia::Pixmap::new(width, height).unwrap();
                    let scale_x = width as f32 / desktop_pixmap.width() as f32;
                    let scale_y = height as f32 / desktop_pixmap.height() as f32;
                    let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);
                    let paint = tiny_skia::PixmapPaint::default();
                    scaled.draw_pixmap(0, 0, desktop_pixmap.as_ref(), &paint, transform, None);
                    scaled
                }
            }
            Some(Err(e)) => {
                println!(
                    "Capture failed ({:?}); refusing to save empty Transparent snapshot",
                    e
                );
                return;
            }
            None => {
                let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
                doc.render_background(&mut pixmap);
                pixmap
            }
        };

        for stroke in &doc.strokes {
            crate::core::render::render_stroke(stroke, &mut temp_pixmap);
        }

        match crate::platform::clipboard::save_and_copy_pixmap(&temp_pixmap, None) {
            Ok((path, copied)) => {
                if copied {
                    println!("Saved Full Screen and copied to clipboard: {}", path);
                } else {
                    println!("Saved Full Screen image to: {} (clipboard copy failed)", path);
                }
            }
            Err(err) => println!("Failed to save image file: {}", err),
        }
    });
}


