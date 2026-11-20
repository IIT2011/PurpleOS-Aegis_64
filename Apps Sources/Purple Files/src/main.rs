#![no_std]
#![no_main]

mod abi;
mod font;
mod fs;
mod gfx;
mod ui;

use core::panic::PanicInfo;
use core::ptr;

use abi::{
    alloc_pages, exit, free_pages, poll_event, present, set_glass, startup_info, window_size,
    yield_cpu, FrameBuffer, EVENT_CLOSE, EVENT_FOCUS, EVENT_OVERFLOW, EVENT_POINTER,
    EVENT_RESIZE, EVENT_KEY,
};
use ui::AppState;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let info = match startup_info() {
        Some(i) => i,
        None => exit(1),
    };

    let handle = info.window_handle;
    let init_w = (info.width as u32).max(320);
    let init_h = (info.height as u32).max(240);

    // Enable Live Aero backdrop glass before first presentation
    let _ = set_glass(handle, true);

    let mut fb = match FrameBuffer::new(init_w, init_h) {
        Some(b) => b,
        None => exit(2),
    };

    // Allocate AppState via 0x1108 so stack usage stays minimal
    let state_ptr = match alloc_pages(core::mem::size_of::<AppState>()) {
        Some(p) => p as *mut AppState,
        None => exit(3),
    };
    unsafe {
        ptr::write(state_ptr, AppState::new(init_w, init_h));
    }
    let app = unsafe { &mut *state_ptr };

    loop {
        let mut had_event = false;
        while let Some(ev) = poll_event(handle) {
            had_event = true;
            match ev.kind {
                EVENT_POINTER => {
                    app.handle_pointer(ev.x, ev.y, ev.detail);
                }
                EVENT_KEY => {
                    app.handle_key((ev.x & 0xFF) as u8);
                }
                EVENT_CLOSE => {
                    app.should_exit = true;
                }
                EVENT_RESIZE => {
                    let (nw, nh) = window_size(handle);
                    if nw > 0 && nh > 0 && fb.resize(nw, nh) {
                        app.resize(nw, nh);
                    }
                }
                EVENT_FOCUS | EVENT_OVERFLOW => {
                    app.dirty = true;
                }
                _ => {}
            }
            if app.should_exit {
                break;
            }
        }

        if app.should_exit {
            break;
        }

        if app.dirty {
            app.render(fb.as_mut_slice());
            let _ = present(handle, fb.as_slice());
        }

        if !had_event {
            app.tick_idle();
            yield_cpu();
        }
    }

    unsafe {
        ptr::drop_in_place(state_ptr);
    }
    free_pages(state_ptr as *mut u8);
    drop(fb);
    exit(0);
}
