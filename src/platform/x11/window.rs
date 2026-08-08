/// X11 window management utilities: scale detection, visual selection, hotkeys, focus, monitor detection.
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    InputFocus, ModMask, GrabMode, Time, ConnectionExt as _,
};

pub fn detect_scale_factor() -> f32 {
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

pub fn find_32bit_visual(screen: &x11rb::protocol::xproto::Screen) -> Option<(x11rb::protocol::xproto::Visualid, u8)> {
    for depth in &screen.allowed_depths {
        if depth.depth == 32 {
            for visual in &depth.visuals {
                return Some((visual.visual_id, 32));
            }
        }
    }
    None
}

pub fn grab_global_hotkeys(conn: &impl Connection, root: u32, keycode_a: u8) {
    // Always-on daemon shortcut. owner_events=true so it still works alongside
    // the overlay key grabs (and while click-through is enabled).
    let modifiers = [
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1),
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1) | u16::from(ModMask::LOCK),
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1) | u16::from(ModMask::M2),
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1) | u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
    ];

    for &mod_mask in &modifiers {
        let _ = conn.grab_key(
            true,
            root,
            mod_mask.into(),
            keycode_a,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        );
    }
}

/// Keep Escape grabbed while the overlay is mapped (including click-through),
/// so Esc always minimizes to the tray without stealing other apps' keys when hidden.
pub fn grab_escape_key(conn: &impl Connection, root: u32, keycode_escape: u8) {
    if keycode_escape == 0 {
        return;
    }
    let modifiers = [
        0u16,
        u16::from(ModMask::LOCK),
        u16::from(ModMask::M2),
        u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
        u16::from(ModMask::ANY),
    ];
    for &mod_mask in &modifiers {
        let _ = conn.grab_key(
            true,
            root,
            mod_mask.into(),
            keycode_escape,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        );
    }
    let _ = conn.flush();
}

pub fn ungrab_escape_key(conn: &impl Connection, root: u32, keycode_escape: u8) {
    if keycode_escape == 0 {
        return;
    }
    let modifiers = [
        0u16,
        u16::from(ModMask::LOCK),
        u16::from(ModMask::M2),
        u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
        u16::from(ModMask::ANY),
    ];
    for &mod_mask in &modifiers {
        let _ = conn.ungrab_key(keycode_escape, root, mod_mask.into());
    }
    let _ = conn.flush();
}

/// Keycodes that should be stolen from the focused app while the overlay is active.
///
/// On GNOME Wayland the overlay runs as an XWayland `override_redirect` window, so
/// `XSetInputFocus` / `XGrabKeyboard` are unreliable. Root `XGrabKey` (same path as
/// Ctrl+Alt+A) is what actually delivers tool shortcuts.
pub fn collect_overlay_keycodes(
    min_keycode: u8,
    max_keycode: u8,
    keysyms: &[u32],
    keysyms_per_keycode: usize,
) -> Vec<u8> {
    let mut out = Vec::new();
    for keycode in min_keycode..=max_keycode {
        let base = ((keycode - min_keycode) as usize) * keysyms_per_keycode;
        let keysym = keysyms.get(base).copied().unwrap_or(0);
        if keysym == 0 || is_modifier_keysym(keysym) {
            continue;
        }
        out.push(keycode);
    }
    out
}

fn is_modifier_keysym(keysym: u32) -> bool {
    matches!(
        keysym,
        // Shift, Caps, Ctrl, Alt/Meta, NumLock, Super/Hyper, Mode_switch, ISO_Level3_Shift…
        0xffe1..=0xffee
            | 0xff7e
            | 0xfe03
            | 0xfe08
            | 0xfe11
            | 0xfe12
            | 0xfe13
    )
}

/// Steal overlay shortcuts/text keys via root grabs (works under XWayland/GNOME).
pub fn grab_overlay_keys(conn: &impl Connection, root: u32, keycodes: &[u8]) {
    // Explicit Lock/NumLock variants (more reliable than ONLY AnyModifier on some XWayland hosts).
    let modifiers = [
        0u16,
        u16::from(ModMask::LOCK),
        u16::from(ModMask::M2),
        u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
        u16::from(ModMask::ANY),
    ];
    for &keycode in keycodes {
        for &mod_mask in &modifiers {
            let _ = conn.grab_key(
                false,
                root,
                mod_mask.into(),
                keycode,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            );
        }
    }
    let _ = conn.flush();
}

/// Release overlay key grabs so typing goes back to the app underneath.
pub fn ungrab_overlay_keys(conn: &impl Connection, root: u32, keycodes: &[u8]) {
    let modifiers = [
        0u16,
        u16::from(ModMask::LOCK),
        u16::from(ModMask::M2),
        u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
        u16::from(ModMask::ANY),
    ];
    for &keycode in keycodes {
        for &mod_mask in &modifiers {
            let _ = conn.ungrab_key(keycode, root, mod_mask.into());
        }
    }
    let _ = conn.flush();
}

pub fn focus_x11_window(conn: &impl Connection, root: u32, win_id: u32) {
    // RevertToParent is the usual choice for overlays that need keyboard focus.
    let _ = conn.set_input_focus(InputFocus::PARENT, win_id, Time::CURRENT_TIME);

    if let Ok(net_active_win) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") {
        if let Ok(reply) = net_active_win.reply() {
            use x11rb::protocol::xproto::{CLIENT_MESSAGE_EVENT, ClientMessageEvent, ClientMessageData, EventMask, ConnectionExt as _};
            let event = ClientMessageEvent {
                response_type: CLIENT_MESSAGE_EVENT,
                format: 32,
                sequence: 0,
                window: win_id,
                type_: reply.atom,
                data: ClientMessageData::from([1u32, u32::from(Time::CURRENT_TIME), 0u32, 0u32, 0u32]),
            };
            let _ = conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
        }
    }
    let _ = conn.flush();
}

/// Invisible 1×1 override-redirect window used only to shed Wayland seat focus.
/// Mapping it, focusing it, then unmapping it releases GNOME's keyboard seat
/// **without** unmapping the visible overlay (no toolbar flicker).
pub fn create_focus_proxy(
    conn: &impl Connection,
    root: u32,
    visual: u32,
    depth: u8,
    colormap: u32,
) -> Result<u32, Box<dyn std::error::Error>> {
    use x11rb::protocol::xproto::{
        CreateWindowAux, EventMask, WindowClass, ConnectionExt as _,
    };

    let win = conn.generate_id()?;
    let aux = CreateWindowAux::new()
        .colormap(colormap)
        .border_pixel(0)
        .background_pixel(0)
        .override_redirect(1)
        .event_mask(EventMask::NO_EVENT);
    conn.create_window(
        depth,
        win,
        root,
        -64,
        -64,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &aux,
    )?;
    let offscreen = [x11rb::protocol::xproto::Rectangle {
        x: -32000,
        y: -32000,
        width: 1,
        height: 1,
    }];
    let _ = x11rb::protocol::shape::rectangles(
        conn,
        x11rb::protocol::shape::SO::SET,
        x11rb::protocol::shape::SK::INPUT,
        x11rb::protocol::xproto::ClipOrdering::UNSORTED,
        win,
        0,
        0,
        &offscreen,
    );
    let _ = conn.flush();
    Ok(win)
}

/// Release keyboard grab/focus so apps, dock, and tray can receive input again.
///
/// On GNOME + XWayland, only unmapping the *focused* X11 surface frees the
/// Wayland seat. We move focus onto the focus proxy, then unmap the proxy —
/// the visible overlay stays mapped (no flicker).
pub fn release_keyboard_focus(
    conn: &impl Connection,
    root: u32,
    overlay_win: u32,
    focus_proxy: u32,
) {
    let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
    set_wm_input_hint(conn, overlay_win, false);
    set_net_wm_user_time(conn, overlay_win, 0);

    if focus_proxy != 0 {
        let _ = conn.map_window(focus_proxy);
        let _ = conn.set_input_focus(InputFocus::PARENT, focus_proxy, Time::CURRENT_TIME);
        let _ = conn.grab_keyboard(
            false,
            focus_proxy,
            Time::CURRENT_TIME,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        );
        let _ = conn.flush();
        let _ = conn.get_input_focus().ok().and_then(|c| c.reply().ok());

        let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
        let _ = conn.unmap_window(focus_proxy);
        let _ = conn.flush();
        let _ = conn.get_input_focus().ok().and_then(|c| c.reply().ok());
    } else {
        let _ = conn.set_input_focus(
            InputFocus::NONE,
            u32::from(InputFocus::POINTER_ROOT),
            Time::CURRENT_TIME,
        );
        let _ = conn.flush();
    }

    if let Some(target) = find_focus_target_under_pointer(conn, root, overlay_win) {
        focus_x11_window(conn, root, target);
    } else {
        let _ = conn.set_input_focus(
            InputFocus::NONE,
            u32::from(InputFocus::POINTER_ROOT),
            Time::CURRENT_TIME,
        );
        let _ = conn.flush();
    }
}

/// ICCCM WM_HINTS.input — false tells the WM not to give us keyboard focus.
pub fn set_wm_input_hint(conn: &impl Connection, win_id: u32, accept_input: bool) {
    use x11rb::properties::WmHints;
    let mut hints = WmHints::new();
    hints.input = Some(accept_input);
    let _ = hints.set(conn, win_id);
}

/// EWMH `_NET_WM_USER_TIME`. Value 0 = map/show without wanting activation.
pub fn set_net_wm_user_time(conn: &impl Connection, win_id: u32, user_time: u32) {
    use x11rb::protocol::xproto::{AtomEnum, PropMode};
    use x11rb::wrapper::ConnectionExt as _;
    let Ok(atom) = conn.intern_atom(false, b"_NET_WM_USER_TIME") else {
        return;
    };
    let Ok(atom) = atom.reply() else {
        return;
    };
    let _ = conn.change_property32(
        PropMode::REPLACE,
        win_id,
        atom.atom,
        AtomEnum::CARDINAL,
        &[user_time],
    );
}

/// Topmost mapped toplevel under the pointer, skipping our overlay.
fn find_focus_target_under_pointer(
    conn: &impl Connection,
    root: u32,
    overlay_win: u32,
) -> Option<u32> {
    let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
    let px = pointer.root_x;
    let py = pointer.root_y;

    // Prefer EWMH stacking (bottom → top).
    let stacking = ewmh_client_list_stacking(conn, root).unwrap_or_default();
    for &win in stacking.iter().rev() {
        if win == 0 || win == overlay_win || win == root {
            continue;
        }
        if window_contains_point(conn, root, win, px, py) {
            return Some(toplevel_of(conn, root, win).unwrap_or(win));
        }
    }

    // Fallback: walk root children (also bottom → top in X11 query_tree).
    let tree = conn.query_tree(root).ok()?.reply().ok()?;
    for &win in tree.children.iter().rev() {
        if win == 0 || win == overlay_win {
            continue;
        }
        if window_contains_point(conn, root, win, px, py) {
            return Some(win);
        }
    }
    None
}

fn ewmh_client_list_stacking(conn: &impl Connection, root: u32) -> Option<Vec<u32>> {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
    let atom = conn
        .intern_atom(false, b"_NET_CLIENT_LIST_STACKING")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let reply = conn
        .get_property(false, root, atom, AtomEnum::WINDOW, 0, 65536)
        .ok()?
        .reply()
        .ok()?;
    Some(reply.value32()?.collect())
}

fn window_contains_point(conn: &impl Connection, root: u32, win: u32, px: i16, py: i16) -> bool {
    let Some(attrs) = conn
        .get_window_attributes(win)
        .ok()
        .and_then(|c| c.reply().ok())
    else {
        return false;
    };
    if attrs.map_state != x11rb::protocol::xproto::MapState::VIEWABLE {
        return false;
    }
    let Some(geom) = conn.get_geometry(win).ok().and_then(|c| c.reply().ok()) else {
        return false;
    };
    let Some(tr) = conn
        .translate_coordinates(win, root, 0, 0)
        .ok()
        .and_then(|c| c.reply().ok())
    else {
        return false;
    };
    let x0 = tr.dst_x;
    let y0 = tr.dst_y;
    let x1 = x0.saturating_add(geom.width as i16);
    let y1 = y0.saturating_add(geom.height as i16);
    px >= x0 && px < x1 && py >= y0 && py < y1
}

fn toplevel_of(conn: &impl Connection, root: u32, mut win: u32) -> Option<u32> {
    for _ in 0..16 {
        let tree = conn.query_tree(win).ok()?.reply().ok()?;
        if tree.parent == 0 || tree.parent == root {
            return Some(win);
        }
        win = tree.parent;
    }
    Some(win)
}

/// Take keyboard focus and grab the keyboard on the overlay window.
/// Call after map, clicks, and stroke end so tool shortcuts keep working on XWayland.
pub fn claim_keyboard(conn: &impl Connection, root: u32, win_id: u32) {
    claim_keyboard_inner(conn, root, win_id, false);
}

/// Same as [`claim_keyboard`] but without console warnings (for frequent re-claims).
pub fn claim_keyboard_quiet(conn: &impl Connection, root: u32, win_id: u32) {
    claim_keyboard_inner(conn, root, win_id, true);
}

fn claim_keyboard_inner(conn: &impl Connection, root: u32, win_id: u32, quiet: bool) {
    use x11rb::protocol::xproto::GrabStatus;
    focus_x11_window(conn, root, win_id);
    match conn.grab_keyboard(
        false,
        win_id,
        Time::CURRENT_TIME,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    ) {
        Ok(cookie) => match cookie.reply() {
            Ok(reply)
                if reply.status == GrabStatus::SUCCESS
                    || reply.status == GrabStatus::ALREADY_GRABBED => {}
            Ok(reply) if !quiet => {
                eprintln!(
                    "WARNING: XGrabKeyboard status={:?}. Click the overlay so tool shortcuts work.",
                    reply.status
                );
            }
            Ok(_) => {}
            Err(e) if !quiet => {
                eprintln!("WARNING: XGrabKeyboard reply failed ({e:?}).");
            }
            Err(_) => {}
        },
        Err(e) if !quiet => {
            eprintln!("WARNING: XGrabKeyboard request failed ({e:?}).");
        }
        Err(_) => {}
    }
    let _ = conn.flush();
}

/// Make an undecorated, always-on-top, focusable overlay window (no override-redirect).
/// Override-redirect windows cannot reliably receive keyboard focus under GNOME/XWayland.
pub fn configure_overlay_wm_hints(conn: &impl Connection, root: u32, win_id: u32) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::properties::{WmHints, WmHintsState};
    use x11rb::protocol::xproto::{AtomEnum, PropMode};
    use x11rb::wrapper::ConnectionExt as _;

    // Accept keyboard input (ICCCM).
    let mut hints = WmHints::new();
    hints.input = Some(true);
    hints.initial_state = Some(WmHintsState::Normal);
    hints.set(conn, win_id)?;

    // No title bar / borders (Motif).
    let motif = conn.intern_atom(false, b"_MOTIF_WM_HINTS")?.reply()?.atom;
    // flags=DECORATIONS (1<<1), decorations=0
    conn.change_property32(PropMode::REPLACE, win_id, motif, AtomEnum::CARDINAL, &[2, 0, 0, 0, 0])?;

    let _ = conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        b"vectrace\0Vectrace\0",
    );
    let _ = conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        b"Vectrace",
    );

    let wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let wm_state_above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
    let wm_state_skip_taskbar = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?.reply()?.atom;
    let wm_state_skip_pager = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")?.reply()?.atom;
    let wm_state_sticky = conn.intern_atom(false, b"_NET_WM_STATE_STICKY")?.reply()?.atom;
    let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
    // NORMAL (not DOCK): GNOME will allow keyboard focus when the window is activated.
    let wm_type_normal = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_NORMAL")?.reply()?.atom;

    conn.change_property32(PropMode::REPLACE, win_id, wm_type, AtomEnum::ATOM, &[wm_type_normal])?;
    conn.change_property32(
        PropMode::REPLACE,
        win_id,
        wm_state,
        AtomEnum::ATOM,
        &[wm_state_above, wm_state_skip_taskbar, wm_state_skip_pager, wm_state_sticky],
    )?;

    let _ = root;
    let _ = conn.flush();
    Ok(())
}

pub fn request_wm_state_above(conn: &impl Connection, root: u32, win_id: u32) {
    use x11rb::protocol::xproto::{CLIENT_MESSAGE_EVENT, ClientMessageEvent, ClientMessageData, EventMask, ConnectionExt as _};
    let Ok(wm_state) = conn.intern_atom(false, b"_NET_WM_STATE") else { return };
    let Ok(wm_state) = wm_state.reply() else { return };
    let Ok(above) = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE") else { return };
    let Ok(above) = above.reply() else { return };

    // _NET_WM_STATE: action=ADD(1), first property=ABOVE
    let event = ClientMessageEvent {
        response_type: CLIENT_MESSAGE_EVENT,
        format: 32,
        sequence: 0,
        window: win_id,
        type_: wm_state.atom,
        data: ClientMessageData::from([1u32, above.atom, 0u32, 1u32, 0u32]),
    };
    let _ = conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    );
    let _ = conn.flush();
}

pub fn detect_primary_monitor(conn: &impl Connection, root: u32) -> Option<(i16, i16, u16, u16)> {
    use x11rb::protocol::randr::ConnectionExt as _;
    if let Ok(cookie) = conn.randr_get_monitors(root, true) {
        if let Ok(reply) = cookie.reply() {
            for mon in &reply.monitors {
                if mon.primary {
                    return Some((mon.x, mon.y, mon.width, mon.height));
                }
            }
            if let Some(first) = reply.monitors.first() {
                return Some((first.x, first.y, first.width, first.height));
            }
        }
    }
    None
}

pub fn keysym_to_char(keysym: u32) -> Option<char> {
    match keysym {
        0x0020..=0x007e | 0x00a0..=0x00ff => char::from_u32(keysym),
        0x01000000..=0x0110ffff => char::from_u32(keysym - 0x01000000),
        _ => None,
    }
}

/// XC_crosshair from X11 cursorfont.h
const XC_CROSSHAIR: u16 = 34;

/// Apply a crosshair cursor for tray region-capture mode.
pub fn set_crosshair_cursor(conn: &impl Connection, win_id: u32) -> Result<u32, Box<dyn std::error::Error>> {
    use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt as _};

    let font = conn.generate_id()?;
    conn.open_font(font, b"cursor")?;

    let cursor = conn.generate_id()?;
    // Source glyph + mask glyph (convention: mask = shape + 1)
    conn.create_glyph_cursor(
        cursor,
        font,
        font,
        XC_CROSSHAIR,
        XC_CROSSHAIR + 1,
        0, 0, 0,
        0xffff, 0xffff, 0xffff,
    )?;
    let _ = conn.close_font(font);

    conn.change_window_attributes(
        win_id,
        &ChangeWindowAttributesAux::new().cursor(cursor),
    )?;
    conn.flush()?;
    Ok(cursor)
}

pub fn clear_window_cursor(conn: &impl Connection, win_id: u32, cursor_id: u32) {
    use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt as _};
    let _ = conn.change_window_attributes(
        win_id,
        &ChangeWindowAttributesAux::new().cursor(0),
    );
    if cursor_id != 0 {
        let _ = conn.free_cursor(cursor_id);
    }
    let _ = conn.flush();
}
