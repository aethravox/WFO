use mmo_protocol::ServerPacket;
use mmo_renderer::MmoClientEngine;
use std::alloc::{alloc, dealloc, Layout};
use std::cell::RefCell;
use std::collections::VecDeque;

thread_local! {
    static ENGINE: RefCell<Option<MmoClientEngine>> = const { RefCell::new(None) };
    static RGBA_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUT_QUEUE: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static LAST_OUT: RefCell<String> = const { RefCell::new(String::new()) };
}

fn with_engine<F: FnOnce(&mut MmoClientEngine) -> R, R>(default: R, f: F) -> R {
    ENGINE.with(|cell| {
        let mut opt = cell.borrow_mut();
        if let Some(eng) = opt.as_mut() {
            f(eng)
        } else {
            default
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let Ok(layout) = Layout::from_size_align(len, 1) else {
        return std::ptr::null_mut();
    };
    unsafe { alloc(layout) }
}

#[no_mangle]
pub extern "C" fn wasm_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    if let Ok(layout) = Layout::from_size_align(len, 1) {
        unsafe { dealloc(ptr, layout) };
    }
}

#[no_mangle]
pub extern "C" fn wasm_init(w: u32, h: u32) {
    let cw = (w as usize).max(320);
    let ch = (h as usize).max(240);
    ENGINE.with(|cell| {
        *cell.borrow_mut() = Some(MmoClientEngine::new(cw, ch));
    });
    RGBA_BUF.with(|buf| {
        buf.borrow_mut().resize(cw * ch * 4, 255);
    });
}

#[no_mangle]
pub extern "C" fn wasm_resize(w: u32, h: u32) {
    let cw = (w as usize).max(320);
    let ch = (h as usize).max(240);
    with_engine((), |eng| {
        eng.resize(cw, ch);
    });
    RGBA_BUF.with(|buf| {
        buf.borrow_mut().resize(cw * ch * 4, 255);
    });
}

#[no_mangle]
pub extern "C" fn wasm_tick_and_render(dt_ms: f32) {
    ENGINE.with(|cell| {
        let mut opt = cell.borrow_mut();
        let Some(eng) = opt.as_mut() else {
            return;
        };
        eng.tick(dt_ms * 0.001);
        OUT_QUEUE.with(|q| {
            let mut queue = q.borrow_mut();
            for pkt in eng.outbox.drain(..) {
                queue.push_back(pkt.encode());
            }
        });
        eng.render();

        RGBA_BUF.with(|rgba_cell| {
            let mut rgba = rgba_cell.borrow_mut();
            let px_count = eng.fb.pixels.len();
            if rgba.len() != px_count * 4 {
                rgba.resize(px_count * 4, 255);
            }
            for (i, &rgb) in eng.fb.pixels.iter().enumerate() {
                let off = i * 4;
                rgba[off] = ((rgb >> 16) & 0xFF) as u8;
                rgba[off + 1] = ((rgb >> 8) & 0xFF) as u8;
                rgba[off + 2] = (rgb & 0xFF) as u8;
                rgba[off + 3] = 255;
            }
        });
    });
}

#[no_mangle]
pub extern "C" fn wasm_rgba_ptr() -> *const u8 {
    RGBA_BUF.with(|buf| buf.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_rgba_len() -> usize {
    RGBA_BUF.with(|buf| buf.borrow().len())
}

#[no_mangle]
pub extern "C" fn wasm_outbox_pop() -> usize {
    OUT_QUEUE.with(|q| {
        if let Some(msg) = q.borrow_mut().pop_front() {
            let len = msg.len();
            LAST_OUT.with(|s| *s.borrow_mut() = msg);
            len
        } else {
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_outbox_ptr() -> *const u8 {
    LAST_OUT.with(|s| s.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn wasm_push_server_msg(ptr: *const u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    if let Ok(line) = std::str::from_utf8(slice) {
        if let Some(pkt) = ServerPacket::decode(line) {
            with_engine((), |eng| eng.apply_server_packet(pkt));
        }
    }
}

#[no_mangle]
pub extern "C" fn wasm_is_typing() -> u32 {
    with_engine(0, |eng| {
        if (!eng.joined
            && (eng.auth_panel_open || eng.lobby_focus_name || eng.lobby_invite_focus))
            || (eng.joined && eng.chat_focused)
        {
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn wasm_key(code: u32, down: u32) {
    let pressed = down != 0;
    with_engine((), |eng| {
        let typing = (!eng.joined
            && (eng.auth_panel_open || eng.lobby_focus_name || eng.lobby_invite_focus))
            || (eng.joined && eng.chat_focused);
        match code {
            1 => eng.input.w = pressed && !typing,
            2 => eng.input.s = pressed && !typing,
            3 => eng.input.a = pressed && !typing,
            4 => eng.input.d = pressed && !typing,
            5 if pressed && !typing => eng.trigger_jump(),
            6 if pressed && !typing => eng.trigger_skill(0),
            7 if pressed && !typing => eng.trigger_skill(1),
            8 if pressed && !typing => eng.trigger_skill(2),
            9 if pressed && !typing => eng.trigger_gather(),
            15 if pressed && !typing => eng.trigger_dodge(),
            13 if pressed && !typing => eng.toggle_personal_marker(),
            14 if pressed && !typing => eng.toggle_bestiary(),
            10 if pressed => eng.on_enter(),
            11 if pressed => eng.on_backspace(),
            12 if pressed => {
                eng.chat_focused = false;
                eng.lobby_focus_name = false;
            }
            _ => {}
        }
    });
}

#[no_mangle]
pub extern "C" fn wasm_char(cp: u32) {
    if let Some(ch) = char::from_u32(cp) {
        with_engine((), |eng| eng.on_char_input(ch));
    }
}

#[no_mangle]
pub extern "C" fn wasm_mouse_down(x: i32, y: i32) {
    with_engine((), |eng| eng.on_pointer_down(x, y));
}

#[no_mangle]
pub extern "C" fn wasm_mouse_move(x: i32, y: i32) {
    with_engine((), |eng| eng.on_pointer_move(x, y));
}

#[no_mangle]
pub extern "C" fn wasm_mouse_up(x: i32, y: i32) {
    with_engine((), |eng| eng.on_pointer_up(x, y));
}

#[no_mangle]
pub extern "C" fn wasm_wheel(dy: f32) {
    with_engine((), |eng| eng.on_wheel(dy));
}

#[no_mangle]
pub extern "C" fn wasm_touch_start(id: u32, x: i32, y: i32) {
    with_engine((), |eng| eng.on_touch_start(id as u64, x, y));
}

#[no_mangle]
pub extern "C" fn wasm_touch_move(id: u32, x: i32, y: i32) {
    with_engine((), |eng| eng.on_touch_move(id as u64, x, y));
}

#[no_mangle]
pub extern "C" fn wasm_touch_end(id: u32, x: i32, y: i32) {
    with_engine((), |eng| eng.on_touch_end(id as u64, x, y));
}

#[no_mangle]
pub extern "C" fn wasm_set_touch_mode(on: u32) {
    with_engine((), |eng| {
        eng.touch_mode = on != 0;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_form_captures_keyboard_input() {
        wasm_init(320, 240);
        assert_eq!(wasm_is_typing(), 1);
    }
}
