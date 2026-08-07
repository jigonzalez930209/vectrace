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

pub fn focus_x11_window(conn: &impl Connection, root: u32, win_id: u32) {
    let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, win_id, Time::CURRENT_TIME);

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
