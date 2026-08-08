pub mod capture;
pub mod window;
pub mod backend;
pub mod render;
pub mod input;

pub use backend::X11Backend;

use crate::core::{Canvas, MonitorMode};
use crate::platform::PlatformBackend;
use crate::ui::Toolbar;

use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    ColormapAlloc, CreateWindowAux, CreateGCAux, EventMask, WindowClass,
    ConnectionExt as _,
};

// --------------------------------------------------------------------------
// Crop selection types (used by backend, render, input submodules)
// --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropHandle {
    TopLeft, TopRight, BottomLeft, BottomRight, Top, Bottom, Left, Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CropDragState {
    None,
    Creating,
    Moving { start_mouse: (f32, f32), initial_rect: (f32, f32, f32, f32) },
    Resizing { handle: CropHandle, initial_rect: (f32, f32, f32, f32) },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropHitResult {
    Handle(CropHandle),
    Inside,
    Outside,
}

use crate::core::Tool;

pub fn hit_test_crop(
    sx: f32, sy: f32, cx: f32, cy: f32,
    px: f32, py: f32, scale: f32,
) -> (CropHitResult, (f32, f32, f32, f32)) {
    let min_x = sx.min(cx);
    let max_x = sx.max(cx);
    let min_y = sy.min(cy);
    let max_y = sy.max(cy);
    let rect = (min_x, min_y, max_x, max_y);
    let margin = 14.0 * scale;

    if (px - min_x).abs() <= margin && (py - min_y).abs() <= margin { return (CropHitResult::Handle(CropHandle::TopLeft), rect); }
    if (px - max_x).abs() <= margin && (py - min_y).abs() <= margin { return (CropHitResult::Handle(CropHandle::TopRight), rect); }
    if (px - min_x).abs() <= margin && (py - max_y).abs() <= margin { return (CropHitResult::Handle(CropHandle::BottomLeft), rect); }
    if (px - max_x).abs() <= margin && (py - max_y).abs() <= margin { return (CropHitResult::Handle(CropHandle::BottomRight), rect); }
    if (py - min_y).abs() <= margin && px >= min_x - margin && px <= max_x + margin { return (CropHitResult::Handle(CropHandle::Top), rect); }
    if (py - max_y).abs() <= margin && px >= min_x - margin && px <= max_x + margin { return (CropHitResult::Handle(CropHandle::Bottom), rect); }
    if (px - min_x).abs() <= margin && py >= min_y - margin && py <= max_y + margin { return (CropHitResult::Handle(CropHandle::Left), rect); }
    if (px - max_x).abs() <= margin && py >= min_y - margin && py <= max_y + margin { return (CropHitResult::Handle(CropHandle::Right), rect); }

    if px > min_x && px < max_x && py > min_y && py < max_y {
        return (CropHitResult::Inside, rect);
    }

    (CropHitResult::Outside, rect)
}

// --------------------------------------------------------------------------
// PlatformBackend impl: the main run() event loop
// --------------------------------------------------------------------------

impl PlatformBackend for X11Backend {
    fn run(&mut self, canvas: &mut Canvas) -> Result<(), Box<dyn Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];

        let root_w = screen.width_in_pixels;
        let root_h = screen.height_in_pixels;

        let primary_mon = window::detect_primary_monitor(&conn, screen.root);
        let (mon_x, mon_y, mon_w, mon_h) = primary_mon.unwrap_or((0, 0, root_w, root_h));

        println!("Detected Primary Monitor: {}x{}+{}+{}", mon_w, mon_h, mon_x, mon_y);
        println!("Virtual Desktop: {}x{}", root_w, root_h);

        let (win_x, win_y, win_w, win_h) = match self.monitor_mode {
            MonitorMode::Primary => (mon_x, mon_y, mon_w, mon_h),
            MonitorMode::All    => (0, 0, root_w, root_h),
        };

        self.width = win_w;
        self.height = win_h;
        self.overlay_x = win_x;
        self.overlay_y = win_y;
        canvas.resize(self.width as u32, self.height as u32);
        canvas.set_scale_factor(self.scale_factor);

        let mut toolbar = Toolbar::new_with_scale(mon_w as f32, self.scale_factor);
        if self.monitor_mode == MonitorMode::All {
            toolbar.x += mon_x as f32;
            toolbar.y += mon_y as f32;
        }

        let (visual_id, depth) = window::find_32bit_visual(screen)
            .unwrap_or((screen.root_visual, screen.root_depth));

        if depth == 32 {
            println!("X11 Transparency enabled (32-bit visual found).");
        } else {
            println!("WARNING: X11 32-bit visual not found. Falling back to default visual (no transparency).");
        }

        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual_id)?;

        let win_id = conn.generate_id()?;
        // On XWayland (GNOME), override_redirect + 32-bit ARGB is what enables a
        // true see-through overlay. Managed/fullscreen Wayland surfaces are opaque
        // by compositor policy. Keyboard focus is claimed via claim_keyboard / grabs.
        let win_aux = CreateWindowAux::new()
            .colormap(colormap)
            .border_pixel(0)
            .background_pixel(0)
            .override_redirect(1)
            .event_mask(
                EventMask::EXPOSURE
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION
                | EventMask::KEY_PRESS
                | EventMask::KEY_RELEASE
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::FOCUS_CHANGE
                | EventMask::ENTER_WINDOW
                | EventMask::LEAVE_WINDOW,
            );

        conn.create_window(depth, win_id, screen.root, win_x, win_y, self.width, self.height, 0, WindowClass::INPUT_OUTPUT, visual_id, &win_aux)?;

        // WM hints are ignored for override-redirect, but harmless if present.
        let _ = window::configure_overlay_wm_hints(&conn, screen.root, win_id);

        self.focus_proxy = window::create_focus_proxy(
            &conn,
            screen.root,
            visual_id,
            depth,
            colormap,
        )?;

        let gc_id = conn.generate_id()?;
        conn.create_gc(gc_id, win_id, &CreateGCAux::new())?;

        conn.map_window(win_id)?;
        let _ = conn.flush();
        // Give XWayland a moment to map before grabbing the keyboard.
        let _ = conn.get_input_focus().ok().and_then(|c| c.reply().ok());
        std::thread::sleep(std::time::Duration::from_millis(50));
        window::claim_keyboard(&conn, screen.root, win_id);

        let min_keycode = conn.setup().min_keycode;
        let max_keycode = conn.setup().max_keycode;
        let keyboard_mapping = conn.get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?.reply()?;
        let keysyms_per_keycode = keyboard_mapping.keysyms_per_keycode as usize;

        let keycode_to_keysym = |keycode: u8, state: u16| -> u32 {
            if keycode < min_keycode || keycode > max_keycode { return 0; }
            let base_idx = ((keycode - min_keycode) as usize) * keysyms_per_keycode;
            let is_shift = (state & 0x0001) != 0; // ShiftMask
            let is_lock  = (state & 0x0002) != 0; // LockMask (CapsLock)
            let use_second_sym = is_shift ^ is_lock;
            let offset = if use_second_sym && keysyms_per_keycode > 1 { 1 } else { 0 };
            let idx = base_idx + offset;
            if idx < keyboard_mapping.keysyms.len() && keyboard_mapping.keysyms[idx] != 0 {
                keyboard_mapping.keysyms[idx]
            } else if base_idx < keyboard_mapping.keysyms.len() {
                keyboard_mapping.keysyms[base_idx]
            } else {
                0
            }
        };

        let mut keycode_a = 0u8;
        let mut keycode_escape = 0u8;
        for kc in min_keycode..=max_keycode {
            let ks = keycode_to_keysym(kc, 0);
            if ks == 0x0061 || ks == 0x0041 { keycode_a = kc; }
            if ks == 0xff1b { keycode_escape = kc; } // XK_Escape
        }
        self.keycode_escape = keycode_escape;

        self.overlay_keycodes = window::collect_overlay_keycodes(
            min_keycode,
            max_keycode,
            &keyboard_mapping.keysyms,
            keysyms_per_keycode,
        );

        if keycode_a > 0 {
            window::grab_global_hotkeys(&conn, screen.root, keycode_a);
            println!("Registered Global Daemon Shortcut: [Ctrl+Alt+A]");
        }
        if keycode_escape > 0 {
            window::grab_escape_key(&conn, screen.root, keycode_escape);
            println!("Registered Escape → System Tray");
        }
        println!(
            "Registered {} overlay key grabs (XWayland-safe tool shortcuts)",
            self.overlay_keycodes.len()
        );

        println!("Controls:\n  [Ctrl+Alt+A] Show from tray / Toggle Click-Through\n  [Space]      Toggle Click-Through\n  [P/H/L/A/R/O/K/N/E/T] Tools\n  [U]          Undo\n  [Ctrl+R]     Redo\n  [C]          Clear canvas\n  [B]          Toggle Background\n  [ESC]        Minimize to System Tray\n");

        // Pre-warm Mutter ScreenCast so the first Save is not a ~4s cold start.
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            match crate::platform::wayland::capture::mutter_ensure_warm() {
                Ok(()) => println!("ScreenCast session pre-warmed (fast screenshots ready)"),
                Err(e) => println!("ScreenCast pre-warm skipped: {:?}", e),
            }
        }

        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
        self.completed_strokes_dirty = true;

        let mut pending_events: Vec<Event> = Vec::new();

        loop {
            // Process tray events
            let mut tray_events = Vec::new();
            if let Some(ref rx) = self.tray_rx {
                while let Ok(ev) = rx.try_recv() { tray_events.push(ev); }
            }

            for tray_event in tray_events {
                use crate::platform::tray::TrayEvent;
                match tray_event {
                    TrayEvent::ToggleVisibility => {
                        let target_hidden = !self.is_hidden;
                        self.set_hidden(&conn, win_id, screen.root, gc_id, canvas, &toolbar, target_hidden)?;
                    }
                    TrayEvent::ToggleSettingsMenu => {
                        self.show_settings_menu = !self.show_settings_menu;
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::ToggleMonitorMode => {
                        self.monitor_mode = self.monitor_mode.toggle();
                        let (win_x, win_y, win_w, win_h) = match self.monitor_mode {
                            MonitorMode::Primary => (mon_x, mon_y, mon_w, mon_h),
                            MonitorMode::All    => (0, 0, root_w, root_h),
                        };
                        self.width = win_w; self.height = win_h;
                        self.overlay_x = win_x;
                        self.overlay_y = win_y;
                        canvas.resize(self.width as u32, self.height as u32);
                        self.x11_pixels.clear();
                        self.completed_strokes_dirty = true;
                        let _ = conn.configure_window(win_id, &x11rb::protocol::xproto::ConfigureWindowAux::new().x(win_x as i32).y(win_y as i32).width(win_w as u32).height(win_h as u32));
                        toolbar = Toolbar::new_with_scale(mon_w as f32, self.scale_factor);
                        if self.monitor_mode == MonitorMode::All { toolbar.x += mon_x as f32; toolbar.y += mon_y as f32; }
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::TogglePassthrough => {
                        self.passthrough = !self.passthrough;
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::CycleBackground => { canvas.cycle_background_mode(); }
                    TrayEvent::ClearCanvas     => { canvas.clear(); }
                    TrayEvent::SaveFull        => { self.trigger_save_full(&conn, win_id, screen.root, canvas, &toolbar); }
                    TrayEvent::SaveRegion      => {
                        self.begin_tray_quick_crop(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::Exit            => { return Ok(()); }
                }
                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
            }

            let event = if let Some(ev) = pending_events.pop() {
                Some(ev)
            } else if let Ok(Some(ev)) = conn.poll_for_event() {
                Some(ev)
            } else {
                None
            };

            if event.is_none() {
                // Wait on the X11 FD instead of spinning with sleep(16).
                // Short timeout keeps laser decay + tray polling responsive.
                let laser_active = canvas.current_stroke().map_or(false, |s| {
                    s.stroke_type == crate::core::StrokeType::Laser && !s.points.is_empty()
                });
                let timeout_ms = if laser_active { 16i32 } else { 50i32 };
                use std::os::fd::AsFd;
                let mut fds = [rustix::event::PollFd::from_borrowed_fd(
                    conn.stream().as_fd(),
                    rustix::event::PollFlags::IN,
                )];
                let _ = rustix::event::poll(&mut fds, timeout_ms);
                if let Some(ref s) = canvas.current_stroke() {
                    if s.stroke_type == crate::core::StrokeType::Laser && !s.points.is_empty() {
                        let dirty_rect = render::get_dirty_rect(canvas, self.width, self.height, None, None);
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                    }
                }
                continue;
            }

            let event = event.unwrap();
            let now_ms = crate::core::canvas::current_time_ms();

            match event {
                Event::Expose(_) => {
                    if !self.passthrough && !self.is_hidden {
                        window::claim_keyboard_quiet(&conn, screen.root, win_id);
                    }
                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                }
                Event::FocusOut(_) => {
                    // GNOME often pulls focus away after pointer interaction; reclaim immediately.
                    if !self.passthrough && !self.is_hidden {
                        window::claim_keyboard_quiet(&conn, screen.root, win_id);
                    }
                }
                Event::FocusIn(_) => {
                    if !self.passthrough && !self.is_hidden {
                        window::claim_keyboard_quiet(&conn, screen.root, win_id);
                    }
                }
                Event::ButtonPress(e) if e.detail == 1 => {
                    let should_exit = {
                        let click_x = e.event_x as f32;
                        let click_y = e.event_y as f32;
                        let toolbar_action = toolbar.handle_click(click_x, click_y, self.show_settings_menu, self.show_color_menu, self.crop_start.is_some() && self.crop_current.is_some());
                        // Check for exit before delegating
                        if let Some(crate::ui::ToolbarAction::Exit) = toolbar_action { true } else { false }
                    };
                    if should_exit { break; }

                    // Also check ToggleMonitorMode since it changes toolbar position
                    let click_x = e.event_x as f32;
                    let click_y = e.event_y as f32;
                    let toolbar_action = toolbar.handle_click(click_x, click_y, self.show_settings_menu, self.show_color_menu, self.crop_start.is_some() && self.crop_current.is_some());
                    if let Some(crate::ui::ToolbarAction::ToggleMonitorMode) = toolbar_action {
                        self.monitor_mode = self.monitor_mode.toggle();
                        let (wx, wy, ww, wh) = match self.monitor_mode {
                            MonitorMode::Primary => (mon_x, mon_y, mon_w, mon_h),
                            MonitorMode::All    => (0, 0, root_w, root_h),
                        };
                        self.width = ww; self.height = wh;
                        self.overlay_x = wx;
                        self.overlay_y = wy;
                        canvas.resize(self.width as u32, self.height as u32);
                        self.x11_pixels.clear(); self.completed_strokes_dirty = true;
                        let _ = conn.configure_window(win_id, &x11rb::protocol::xproto::ConfigureWindowAux::new().x(wx as i32).y(wy as i32).width(ww as u32).height(wh as u32));
                        toolbar = Toolbar::new_with_scale(mon_w as f32, self.scale_factor);
                        if self.monitor_mode == MonitorMode::All { toolbar.x += mon_x as f32; toolbar.y += mon_y as f32; }
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    } else {
                        self.handle_button_press(&conn, win_id, screen.root, gc_id, canvas, &toolbar, click_x, click_y, now_ms)?;
                    }
                }
                Event::MotionNotify(e) => {
                    let mut move_x = e.event_x as f32;
                    let mut move_y = e.event_y as f32;
                    while let Ok(Some(next)) = conn.poll_for_event() {
                        if let Event::MotionNotify(ne) = next { move_x = ne.event_x as f32; move_y = ne.event_y as f32; }
                        else { pending_events.push(next); break; }
                    }
                    self.handle_motion(&conn, win_id, gc_id, canvas, &mut toolbar, move_x, move_y, now_ms)?;
                }
                Event::ButtonRelease(e) if e.detail == 1 => {
                    self.handle_button_release(&conn, win_id, screen.root, gc_id, canvas, &toolbar)?;
                }
                Event::KeyPress(e) => {
                    let keysym = keycode_to_keysym(e.detail, e.state.into());
                    let should_exit = self.handle_key_press(&conn, win_id, screen.root, gc_id, canvas, &toolbar, keysym, e.state.into(), e.detail, keycode_a, now_ms)?;
                    if should_exit { break; }
                }
                Event::ConfigureNotify(e) => {
                    if e.width != self.width || e.height != self.height {
                        self.width = e.width;
                        self.height = e.height;
                        canvas.resize(self.width as u32, self.height as u32);
                        toolbar = Toolbar::new_with_scale(self.width as f32, self.scale_factor);
                        self.x11_pixels.clear();
                        self.completed_strokes_dirty = true;
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
