# 22-11-2026 - Compositor Mouse Drag Latch Fix, App Lifecycle Cleanup, and Global Bar Persistence

**Component:** Native Compositor (`src/compositor/mod.rs`), Application Runtime & Syscalls (`src/kernel/app_runtime.rs`), Native Terminal (`Apps Sources/Terminal/src/purple.rs`), VLC Media Player (`References/vlc/bin/purple_vlc.c`)  
**Context:** Verification of macOS Global Application Bar & Dock persistence after closing VLC; diagnosis and resolution of mouse freezing when opening and closing Terminal.

---

## 1. Problem Statement & Root Cause Analysis

### A. macOS Global Bar & Dock Persistence after Closing VLC
* **Inquiry:** User requested verification of whether the global bar and dock disappear after VLC is closed.
* **Findings:** In the native compositor, the top bar is rendered persistently by `desktop::render_desktop_shell` at `y = 0..BAR_HEIGHT`. When VLC opens, `active_app` becomes `Some("VLC Media Player")`, displaying VLC's application menus (`Media`, `Playback`, `Audio`, `Video`, `Subtitle`, `Tools`, `View`, `Help`).
* When VLC closes (`window close 1` or close button), `active_app` transitions back to `None`, cleanly restoring the default macOS Finder menus (`Finder`, `File`, `Edit`, `View`, `Go`, `Window`, `Help`). Pixel inspection confirms the acrylic panel and dock remain 100% intact.

### B. Mouse Freezing When Opening and Closing Terminal
The user reported that the mouse froze when opening and closing Terminal. Deep investigation across QEMU execution traces, compositor state machines, and task logs identified 4 interconnected root causes:

1. **Compositor Drag State Latch Bug (`src/compositor/mod.rs:599`):**
   * `self.dragging_window_id` and `self.resize_gesture` were previously only reset upon an edge event `left_released` (`(buttons & 0x01) == 0 && (self.last_mouse_buttons & 0x01) != 0`).
   * If a mouse packet delivered `buttons == 0` without matching `last_buttons != 0` (such as during packet coalescing or when a window closed during click), `self.dragging_window_id` remained stuck on `Some(id)`.
   * In `handle_mouse_event`, when `dragging_window_id.is_some()`, the compositor entered the drag handler, searched for the window (which was already destroyed), found nothing, rendered, and executed an early `return;`.
   * As a result, all future mouse movement events skipped hit-testing and event dispatch, swallowing all clicks and leaving the cursor latched in grabbing mode forever.

2. **Orphaned Window on Client Exit (`Apps Sources/Terminal/src/purple.rs:934`):**
   * When the traffic-light Close Button was clicked, the compositor dispatched `EVENT_CLOSE` (event 4).
   * Terminal responded by invoking `exit(0)` (`SYS_EXIT`) without closing its window handle via `SYS_WINDOW_CLOSE` (0x1103).
   * The compositor retained the window in `windows` with `focused_window_id = Some(win)` and `owner = Some(dead_pid)`.

3. **Terminal Infinite Yield Loop on Destruction (`purple.rs:780`):**
   * If the window was destroyed by the compositor or shell, `syscall(0x1105, window)` (`SYS_WINDOW_POLL`) returned `-EBADF` (-9).
   * Terminal checked `while syscall(...) == 1`, dropping out without error handling, and repeatedly invoked `syscall(24)` (`SYS_SCHED_YIELD`), burning 100% CPU on Core 0 in a tight loop pinned to the same core as AegisShell.

4. **Kernel Missing Automatic Window Cleanup (`src/kernel/app_runtime.rs:320`):**
   * On `SYS_EXIT` and `SYS_EXIT_GROUP`, `app_runtime` called `task::exit()` without checking or closing windows owned by the terminating task.

5. **Terminal Stack Allocation Overflow:**
   * `purple_terminal_main` allocates a 114 KiB stack frame for transcripts, inputs, prompts, and buffers.
   * `terminal.apapplication` was bundled with the default 64 KiB stack, causing the compiler's stack probe (`movq $0, (%rsp)`) to trigger a Page Fault (`RIP 0x80000028f4 ERR 0x6`).

---

## 2. Engineering Changes

### A. Compositor Mouse State Invariant (`src/compositor/mod.rs`)
* Enforced the unconditional rule: if `(buttons & 0x01) == 0`, `self.dragging_window_id` and `self.resize_gesture` are reset to `None`.
* In both the motion drag handler and resize gesture handler, if the target window ID no longer exists in `self.windows`, the gesture is immediately cleared and does not return early.
* In `WindowHitArea::CloseButton`: cleared `dragging_window_id` and `resize_gesture`, verified task liveness with `native_alive()`, and immediately destroyed windows of terminated processes.
* In `service()`: added queue check (`POINTER_EVENTS.lock().1 > 0`) so pointer events are never delayed when `client_frame_pending` is false, and called `reap_client_windows()` before and after mouse event processing.

### B. Kernel Auto-Reap on Process Exit (`src/kernel/app_runtime.rs`)
* Updated `SYS_EXIT` and `SYS_EXIT_GROUP` dispatch: iterates through `COMPOSITOR.lock().windows` and invokes `engine.close_window(id)` for all windows owned by the exiting process before calling `task::exit()`.

### C. Terminal Event Loop & Clean Close (`Apps Sources/Terminal/src/purple.rs`)
* Wrapped `SYS_WINDOW_POLL` (0x1105) in a loop with explicit error checks: if `res < 0`, Terminal calls `exit(0)` immediately.
* On event 4 (`EVENT_CLOSE`), Terminal explicitly issues `syscall(0x1103, window, 0, 0)` before exiting.

### D. Terminal Stack Size Expansion (`scripts/package_gui_app.py`)
* Repackaged `terminal.apapplication` with `--stack 262144` (256 KiB stack), providing ample headroom for the 114 KiB stack frame and eliminating all user-mode page faults.

### E. VLC Graceful Exit Checks (`References/vlc/bin/purple_vlc.c`)
* Added `poll_res < 0` and `purple_gui_present < 0` detection to cleanly stop the Intel HDA audio stream and exit if the window is closed externally.

---

## 3. Verification & Test Results

Executed automated QEMU test suite [OS/scripts/test_vlc_close_and_terminal.py](file:///data/data/com.termux/files/home/OS%20Development/OS/scripts/test_vlc_close_and_terminal.py):
1. **Desktop Boot:** Verified macOS global bar and dock active (`01_clean_desktop.png`).
2. **VLC Launch & Close:** VLC window launched, audio streamed, top bar showed VLC menus (`02_vlc_open.png`). Closed with `window close 1`; audio halted and top bar restored to Finder (`03_vlc_closed.png`).
3. **Terminal Launch:** Terminal opened cleanly without page faults (`PID 7`, 780x500, `04_terminal_open.png`).
4. **Close Button Click:** Moved mouse to (888, 112) and simulated physical left click. Window destroyed, Terminal task reaped cleanly, no zombie tasks, no spin loop (`05_terminal_closed_by_close_button.png`).
5. **Mouse Fluidity Verification:** Moved mouse across desktop to (638, 312) and sent `echo MOUSE_IS_RESPONSIVE`. Shell responded instantly, cursor position updated, zero freezing (`06_mouse_responsive_after_close.png`).
6. **Re-Open & Shell Close:** Reopened Terminal (`PID 8`, window ID 3), closed via `window close 3`, verified clean reap and full mouse responsiveness (`07_terminal_reopened_and_closed_via_shell.png`).
