use crate::core::{Canvas, Point, Tool, StrokeType};
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

pub struct WaylandState {
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub layer_shell: Option<ZwlrLayerShellV1>,
    pub seat: Option<wl_seat::WlSeat>,
    pub surface: Option<wl_surface::WlSurface>,
    pub layer_surface: Option<ZwlrLayerSurfaceV1>,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    
    pub pointer_x: f64,
    pub pointer_y: f64,
    pub button_pressed: bool,
    pub last_button_time: u32,
    
    pub pending_key: Option<(u32, u32)>, // keycode, state
}

impl WaylandState {
    pub fn new() -> Self {
        Self {
            compositor: None,
            shm: None,
            layer_shell: None,
            seat: None,
            surface: None,
            layer_surface: None,
            pointer: None,
            keyboard: None,
            configured: false,
            width: 0,
            height: 0,
            pointer_x: 0.0,
            pointer_y: 0.0,
            button_pressed: false,
            last_button_time: 0,
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

pub struct WaylandBackend {
    passthrough: bool,
    active_tool: Tool,
    scale_factor: f32,
}

impl WaylandBackend {
    pub fn new() -> Self {
        let scale_factor = detect_scale_factor();
        Self {
            passthrough: false,
            active_tool: Tool::default_pen(),
            scale_factor,
        }
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
        let layer_shell = match globals.bind::<ZwlrLayerShellV1, _, _>(&qh, 1..=4, ()) {
            Ok(ls) => ls,
            Err(e) => {
                println!("Wayland layer-shell extension is not supported by this compositor ({:?}).", e);
                println!("Falling back to XWayland / X11 overlay backend...");
                let mut x11_backend = crate::platform::x11::X11Backend::new();
                return x11_backend.run(canvas);
            }
        };
        
        if let Ok(seat) = globals.bind::<wl_seat::WlSeat, _, _>(&qh, 1..=7, ()) {
            state.pointer = Some(seat.get_pointer(&qh, ()));
            state.keyboard = Some(seat.get_keyboard(&qh, ()));
            state.seat = Some(seat);
        }

        state.compositor = Some(compositor.clone());
        state.shm = Some(shm.clone());
        state.layer_shell = Some(layer_shell.clone());

        let surface = compositor.create_surface(&qh, ());
        state.surface = Some(surface.clone());

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
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

        surface.commit();
        event_queue.roundtrip(&mut state)?;

        while !state.configured {
            event_queue.blocking_dispatch(&mut state)?;
        }

        let width = if state.width > 0 { state.width } else { 1920 };
        let height = if state.height > 0 { state.height } else { 1080 };

        canvas.resize(width, height);
        canvas.set_scale_factor(self.scale_factor);
        println!("Wayland Multimonitor Overlay initialized: {}x{} (Scale: {:.1}x)", width, height, self.scale_factor);

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

        apply_wayland_passthrough(&compositor, &surface, self.passthrough, &toolbar, &qh);

        loop {
            event_queue.dispatch_pending(&mut state)?;

            let cur_x = state.pointer_x as f32;
            let cur_y = state.pointer_y as f32;
            let now_ms = crate::core::canvas::current_time_ms();

            if state.button_pressed && !prev_button_pressed {
                if let Some(action) = toolbar.handle_click(cur_x, cur_y) {
                    if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                        canvas.finish_current_stroke();
                    }

                    match action {
                        ToolbarAction::SelectTool(tool) => {
                            self.active_tool = tool;
                            println!("Selected tool: {:?}", tool);
                        }
                        ToolbarAction::SelectShape(kind) => {
                            self.active_tool = Tool::default_shape(kind);
                            println!("Selected shape: {:?}", kind);
                        }
                        ToolbarAction::SetColor(color) => {
                            self.active_tool.set_color(color);
                            println!("Set color: {:?}", color);
                        }
                        ToolbarAction::ToggleBackgroundMode => {
                            let mode = canvas.cycle_background_mode();
                            println!("Switched background mode to: {:?}", mode);
                        }
                        ToolbarAction::Clear => {
                            canvas.clear();
                            println!("Canvas cleared");
                        }
                        ToolbarAction::TogglePassthrough => {
                            self.passthrough = !self.passthrough;
                            println!("Toggled Click-Through: {}", self.passthrough);
                            apply_wayland_passthrough(&compositor, &surface, self.passthrough, &toolbar, &qh);
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
                    match key_code {
                        1 => {
                            if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                                canvas.cancel_current_stroke();
                            } else {
                                println!("Exiting...");
                                break;
                            }
                        }
                        57 => {
                            self.passthrough = !self.passthrough;
                            println!("Toggled Click-Through: {}", self.passthrough);
                            apply_wayland_passthrough(&compositor, &surface, self.passthrough, &toolbar, &qh);
                        }
                        48 => {
                            let mode = canvas.cycle_background_mode();
                            println!("Switched background mode to: {:?}", mode);
                        }
                        22 => {
                            if canvas.undo() {
                                println!("Undo stroke");
                            }
                        }
                        19 => {
                            if canvas.redo() {
                                println!("Redo stroke");
                            }
                        }
                        46 => {
                            canvas.clear();
                            println!("Canvas cleared");
                        }
                        28 => {
                            if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                                canvas.finish_current_stroke();
                                println!("Committed text");
                            }
                        }
                        _ => {}
                    }
                }
            }

            canvas.render_background(&mut base_pixmap);
            canvas.render_completed_strokes(&mut base_pixmap);
            canvas.render_current_stroke(&mut base_pixmap);
            toolbar.draw(&mut base_pixmap, self.active_tool, self.passthrough, canvas.background_mode);

            let shm_slice = unsafe {
                std::slice::from_raw_parts_mut(mmap as *mut u8, buffer_size)
            };
            shm_slice.copy_from_slice(base_pixmap.data());

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
    passthrough: bool,
    toolbar: &Toolbar,
    qh: &QueueHandle<WaylandState>,
) {
    if passthrough {
        let region = compositor.create_region(qh, ());
        region.add(
            toolbar.x as i32,
            toolbar.y as i32,
            toolbar.width as i32,
            toolbar.height as i32,
        );
        surface.set_input_region(Some(&region));
        region.destroy();
    } else {
        surface.set_input_region(None);
    }
    surface.commit();
}
