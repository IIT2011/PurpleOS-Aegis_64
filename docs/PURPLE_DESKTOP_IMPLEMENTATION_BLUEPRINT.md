# Purple Desktop Implementation Blueprint

**Status:** Implementation contract for future desktop work

**Target:** PurpleOS / Aegis64, `x86_64-unknown-none`, bare metal, `no_std`

**Scope:** Native graphical desktop shell and compositor foundation

**Design reference:** `docs/PURPLE_DESKTOP_UI_SPECIFICATION.md`

**Last reviewed:** 2026-11-18

This document is the implementation handoff for an engineer or AI agent building the Purple Desktop. It describes intended behavior, interfaces, constraints, and completion criteria. It does not claim that planned features already exist. When this document conflicts with a UI concept document, the design tokens and visible composition in that document remain the visual authority; this blueprint is the authority for implementation behavior and current-state facts. Existing code and hardware facts must be rechecked before implementation.

## 1. Product definition

Purple Desktop is the native 2D graphical session for PurpleOS. It runs inside the kernel over the active linear framebuffer and presents a wallpaper, a floating top bar, a bottom application dock, and client windows. It must work at the framebuffer dimensions provided at boot; it must not assume a particular monitor resolution. It is not an X server, Wayland compositor, Java desktop, or hosted Linux window manager.

The visual identity is deep violet space, restrained translucent panels, clear window hierarchy, and a small amount of spatial motion and reflection. Looking Glass is a source of high-level interaction ideas only. Purple Desktop is an original, simple, pixel-based system suited to a `no_std` kernel.

### Current implementation status (reviewed 2026-11-18)

Implemented: boot-time framebuffer discovery where supplied by the boot protocol; a compositor engine; a compiled RAW nebula wallpaper rendered with aspect-preserving center crop to the active surface; basic ARGB8888 window surfaces and titlebars; basic z-order, focus, move, minimize, maximize, close, and software cursor handling; framebuffer presentation; a shell command path for compositor demonstration windows.

Not implemented: the Aegis Bar, Flow Dock, launcher, desktop session model, managed application lifecycle, GUI client event dispatch, app surface protocol, drag-to-resize, animated transitions, real tray integration, a periodic compositor frame loop, configured desktop theme loading, true synchronized page flipping, or a 60 Hz guarantee. `DamageTracker` exists but is not connected to partial redraw. The current compositor starts without demo windows; the demo is opt-in through the shell/VFS command.

`src/kernel/apexec.rs` currently only announces the dynamic execution engine; it is not an application loader. `docs/APAPPLICATION_SPECIFICATION.md` is a proposed contract, not an implemented API. `build/mkpfs2_disk` seeds desktop and compositor configuration files, but the compositor does not currently consume them. The shell currently checks keyboard characters before serial input, despite journal descriptions of a serial-only shell. Reconcile that behavior when the desktop takes keyboard input. Existing journal and UI-spec statements about double buffering, VSync, anti-aliased fonts, damage rendering, and completed desktop features are design intent unless verified against the code.

## 2. Clean-room use of Looking Glass and licensing

The locally reviewed `lg3d-core` source dump is a read-only design reference, not a PurpleOS build dependency. Its root `LICENSE` is GNU GPL version 2. Individual source files contain their own copyright and license headers; review each file and all assets separately before any proposed reuse. The reviewed `Frame3D.java` header names Sun Microsystems copyright and GPLv2. No Classpath Exception was found in the reviewed root license or that header. A previous journal's “GPLv2 Classpath” wording is not supported by this review and must not be used as a licensing conclusion.

For this implementation, use clean-room practice: study documented behavior and broad interaction patterns, then design and implement PurpleOS-native data structures and rendering independently. Do not copy, translate, mechanically port, or imitate source structure, method bodies, API names, Java/Java3D types, original images, icons, textures, sounds, or other bundled resources. Do not make the local source tree a dependency. If reuse is ever proposed, stop and perform a file-by-file provenance and license review before including it. Keep a provenance note for newly added artwork, fonts, and glyphs.

Reviewed concepts and files in the local reference tree:

| Looking Glass reference | General concept to learn | PurpleOS treatment |
|---|---|---|
| `src/classes/org/jdesktop/lg3d/wg/Frame3D.java` | A top-level application surface has a lifecycle and visible state | Define an original Rust `WindowId` and window lifecycle |
| `src/classes/org/jdesktop/lg3d/scenemanager/SceneManager.java` | Scene ownership is separate from individual app surfaces | A kernel-owned desktop session owns shell layers and windows |
| `src/classes/org/jdesktop/lg3d/scenemanager/CursorModule.java` | Pointer position and target picking belong to a coordinated input layer | Route pointer events through hit testing and pointer capture |
| `src/classes/org/jdesktop/lg3d/scenemanager/utils/taskbar/GlassyTaskbar.java` and related taskbar classes | A task shelf can expose running tasks and launch shortcuts | Create an original dock with launch entries and running indicators |
| `src/classes/org/jdesktop/lg3d/wg/AnimationScheduler.java` | Timed animation work benefits from central scheduling | Use bounded desktop timers/ticks; no per-widget busy loops |
| `src/classes/org/jdesktop/lg3d/utils/eventadapter/MouseClickedEventAdapter.java` | Input events can trigger decoupled actions | Map PurpleOS input events to explicit desktop actions |
| `src/classes/org/jdesktop/lg3d/wg/Component3D.java`, `Container3D.java`, `Animation*` | Reusable components and containers help organize a scene | Use small original 2D widgets over raster surfaces; no 3D scene graph |

These references are conceptual pointers, not required reading during a build and not a permission grant. PurpleOS owns the implementation and its names, visuals, and interfaces.

## 3. System boundaries and architecture

```text
PS/2 mouse IRQ ----> bounded input queue ----+
Keyboard input ----> bounded input queue -----+--> DesktopSession / event router
Serial input -------------------------------> shell (diagnostics and text CUI)
                                                   |
                                                   +--> window manager / launcher / widgets
                                                   |       wallpaper, bar, dock, windows
                                                   +--> compositor scene and render scheduler
                                                            |
                                                Surface + blend + cursor
                                                            |
                                          active VIDEO/GPU framebuffer presentation
```

The kernel owns display memory and the desktop scene. The compositor is a renderer, not the application manager. The desktop session owns shell widgets, focus, window state, launcher state, and task indicators. Applications request windows and receive input through a controlled interface; they never write directly to the physical framebuffer.

Keep these responsibilities distinct, even if the first version is a few modules:

1. **Display backend:** discover and validate the boot-provided framebuffer, dimensions, pitch, pixel format, and accessible memory. Present a completed frame. Report failure if no usable display exists.
2. **Input producers:** decode hardware packets into small events. They do not call UI code or render.
3. **Input queue/router:** preserve key and button ordering, coalesce redundant pointer motion when full, and deliver events to the desktop session.
4. **Desktop session:** owns bar, dock, launcher, windows, focus, pointer capture, task state, and actions.
5. **Window manager:** validates geometry, z-order, state transitions, hit testing, clipping, and client event delivery.
6. **Compositor:** paints the desktop layers and window surfaces into a back surface, then presents it.
7. **Application service:** initially launches built-in kernel-resident clients. A future user-mode loader and IPC protocol must be designed separately from this UI foundation.

No external display server, Wayland, X11, Java runtime, or new crate dependency is part of this foundation. Avoid allocation in interrupt context. Use bounded queues and checked geometry arithmetic appropriate for a bare-metal kernel.

## 4. Existing implementation map and gaps

| Existing file | Current role | Foundation work still needed |
|---|---|---|
| `src/compositor/mod.rs` | Engine, window collection, focus and basic operations | Session lifecycle, frame scheduling, queue-driven input, shell layers |
| `src/compositor/pipeline.rs` | Wallpaper, windows, cursor render order | Background/foreground desktop widgets, clipping, fallback and frame policy |
| `src/compositor/window.rs` | Window geometry, chrome, hit areas | Resize interaction, robust constraints, input delivery, richer lifecycle |
| `src/compositor/surface.rs` | ARGB surface, rects, primitives, damage region helper | Safe client ownership and damage integration |
| `src/compositor/blend.rs` | Source-over blend and simple shadow/gradient helpers | Validate alpha convention; optimize only after correctness |
| `src/compositor/cursor.rs` | Software pointer bitmap and save/restore support | Integrate exclusively in final pass; avoid stale restore artifacts |
| `src/compositor/wallpaper.rs` | Embedded RAW wallpaper with dynamic aspect-fit/crop | Preserve dimension-driven behavior and define fallback on decode failure |
| `src/drivers/mouse.rs` | PS/2 packet decode and pointer handling | IRQ must enqueue only; move compositor calls out of event/IRQ path |
| `src/drivers/keyboard.rs` | Character-oriented input ring | Add GUI key/modifier events while preserving shell input |
| `src/kernel/shell.rs` | Serial and keyboard command shell | Explicitly route keyboard to desktop when active; retain serial diagnostics |
| `src/kernel/apexec.rs` | Startup log placeholder | Do not treat as a loader; future application service is separate work |
| `build/mkpfs2_disk` | Seeds configuration and system files | Define and implement a parser/consumer, or document defaults as authoritative |

### Current display truth

Use the active framebuffer dimensions and pixel format; never use the GPU driver's fallback mode as a desktop layout constant. The boot framebuffer is the source of truth when valid. The current code copies/presents frames; the blueprint must not call that a page flip or synchronized VSync. Until a real frame timer and hardware synchronization are implemented, render on state changes and report measured capability honestly. A missing or invalid framebuffer should leave the text/serial paths available and defer desktop startup rather than inventing a display size.

## 5. Desktop composition and visual contract

The design tokens and positions below preserve the approved visual specification. They are design-space starting values, not assumptions about physical resolution. Compute every placement from current framebuffer width and height, scale, and safe bounds.

### Layer order

Paint each complete frame in this order (back to front):

1. Opaque wallpaper/background, scaled to the full active display using the existing aspect-preserving center-crop behavior.
2. Optional dock reflection beneath the dock, clipped to display bounds. It may be omitted on low capability or when it would overlap content at small heights.
3. Window shadows and windows, ordered by increasing z value; the focused window is topmost among ordinary windows.
4. Aegis Bar and its controls.
5. Dock and running-task indicators.
6. Popups, launcher, tooltips, and menus with an explicit overlay priority.
7. Cursor, always last.

Shell UI must not become an ordinary application window. It stays above windows and below transient popups/cursor. Hit testing follows the reverse visual order so the topmost eligible element wins.

### Aegis Bar

- Floating panel near the top edge, design height 32 px, 12 px top inset, 20 px horizontal inset, rounded corners. Clamp margins/radius to the available display bounds.
- Left: original Purple “P” launcher control. Click opens/closes the launcher; keyboard activation is supported when focused.
- Center: current clock in `HH:MM UTC` format. Use the kernel's real clock source when available. If unavailable, show a neutral unavailable state; never display a fabricated time.
- Right: status indicators only when backed by actual subsystem state. Network, audio, and power/battery currently may be absent; omit unsupported indicators or render a clear unavailable state. Do not hardcode “connected” or invent telemetry.
- Base surface color `#CC160B2A`, subtle highlight, border and text tokens from the UI specification. In the initial renderer, translucent color is alpha compositing, not a promise of background blur.

### Purple Flow Dock

- Centered near the bottom edge, design panel height 56 px and 24 px bottom inset. Five design entries: Terminal, Files, System Monitor, Code Studio, Settings. Each design icon is 40 by 40 px; entries may shrink or become a scrollable/overflow list if the display cannot fit them.
- Launch entries are metadata, not proof those applications exist. Disabled/unavailable entries must communicate their state and must not launch a fake executable.
- Running applications display a small violet indicator. Minimized windows remain represented and can be restored. Clicking an active entry focuses its most recent window; clicking a minimized one restores it; clicking an inactive entry requests launch if registered.
- Reordering and magnification are later enhancements. Reflection is optional and subordinate to legibility; never let it intercept pointer input.

### Window chrome and palette

Preserve the UI specification tokens: canvas `#140824` / `#1B0C30`; glass base `#CC160B2A`; border `#44C084FC`; amethyst `#A855F7`; violet `#C084FC`; cyan `#38BDF8`; primary text `#F8FAFC`; muted text `#94A3B8`.

Use a 28 px titlebar, readable title, and close/minimize/maximize controls. Window edges and shadows must remain distinguishable on the nebula background. The current implementation has a 28 px titlebar, 2 px border, and 8 px shadow; design specification's 1 px border and 16 px shadow are not current facts. Keep initial effects inexpensive and clamp all drawing to surface bounds. A maximize target uses the usable desktop area between shell insets, not the entire physical screen. Restore returns the prior normal rectangle.

## 6. Resolution-independent layout

Read width, height, pitch, and format from the selected framebuffer at session start. Re-read or receive a display-change event if mode changes are ever supported. All rectangles use checked signed coordinates internally and are clipped before memory access.

For the initial shell, use design-space constants scaled from a 1024 by 768 reference canvas, but never force the display to that mode. Derive a uniform scale as `clamp(min(width / 1024, height / 768), 0.65, 1.5)` and then clamp individual panel dimensions against actual available bounds. This scale is a starting rule for the first implementation, not a mode setting. If dimensions are too small to support the full design, reduce gaps and icon size down to a declared minimum, then put dock entries behind an overflow affordance. The bar keeps its three regions; center the clock in the actual bar independently of left/right content widths.

Window defaults are proportional: initial width is 55% of usable width, initial height 60% of usable height, each clamped to configured minimums and the available work area. Center the first window in the work area. For narrow/short displays, minimum dimensions are capped to the work area so a window remains reachable. Titlebar controls retain adequate click targets after scaling. Wallpaper scaling is independent from UI scaling and remains aspect-preserving center crop; no display mode is hardcoded.

All inset and geometry calculations must handle zero dimensions, integer overflow, and work areas smaller than preferred controls. Layout changes trigger a full invalidation before the next presentation.

## 7. Window state and interaction contract

Define an original, stable identifier (`WindowId`) and a bounded collection sized for kernel memory. A window record should contain owner/app identity, title, normal rectangle, current rectangle, minimum size, state, z rank, surface handle, input eligibility, and dirty region. The first implementation may use fixed-size arrays and a constant capacity, but expose capacity through status rather than silently corrupting state.

Required states: `Normal`, `Maximized`, `Minimized`, and `Closing` (or equivalent). Required transitions:

- Create: validate owner, title length, surface dimensions, and geometry; place within work area; focus and raise it.
- Focus: raise to top, update focused decoration and keyboard target, and leave shell overlays above it.
- Move: begin only from titlebar drag area outside buttons; capture pointer until release; keep enough of titlebar visible to recover the window.
- Resize: hit-test edges/corners, capture pointer, enforce minimum size and work-area constraints, and invalidate old and new extents. Existing hit testing recognizes a resize border but does not perform resizing.
- Minimize: hide surface from window pass, retain task entry, release input focus, and focus the next eligible window. Dock activation restores it.
- Maximize/restore: save and restore normal geometry; maximize to usable work area.
- Close: request client shutdown; after completion or bounded timeout, release surface and task state. A built-in client can close synchronously. Do not let a client callback run while holding the scene/window collection lock.
- Destroy: release resources exactly once and choose a valid next focus target.

Hit test topmost window first; test controls before titlebar/client; then shell elements according to overlay priority. Pointer capture is explicit and released on button-up even if the pointer leaves the original rectangle. Deliver client mouse/key events to the focused, visible, input-enabled window. The current implementation has no client event protocol, so first wire built-in desktop clients through the kernel-owned API.

Clipping is mandatory for windows partly off-screen, shadows, text, and client pixels. Z ranks must be renormalized safely before integer wrap. No single application can cover the cursor or shell overlays.

## 8. Input, interrupt, and scheduling rules

The IRQ/packet path may acknowledge hardware, decode a packet, and append a compact event to a bounded queue. It must not render, allocate, block, call application code, or acquire a lock that can be held by the compositor. This is especially important because the current mouse path can synchronously invoke compositor work from polling called by the interrupt handler.

Represent events with explicit types such as pointer move, button transition, key transition, modifier update, and focus/resize notification. Preserve key/button order. When the queue is full, pointer moves may be coalesced to the latest position; never silently discard key presses or button edges. If the queue remains full, track overflow and recover safely (for example, reconcile button state from hardware on the next poll).

The compositor/session owner drains queued events outside interrupt context, updates model state, marks dirty areas, and schedules a render. No scene lock is held while invoking a client or renderer callback. If the kernel has no scheduler task suitable for this yet, add a bounded poll/drain point in the main loop; do not make the IRQ perform the UI work as a shortcut.

When desktop session is active, PS/2 keyboard events go to the focused GUI client first. Serial remains available for diagnostics and the CUI shell. Provide a documented mode switch or shell command to select text shell input if needed. Do not consume the same keyboard byte in both desktop and shell paths. Preserve early boot input before desktop activation through the existing shell path.

Use the existing kernel tick source where available. Target at most 60 frames per second for animation, but render immediately for visible input when practical and coalesce bursts. If no periodic scheduler is ready, static desktop frames render only when invalidated. Time-based animation uses elapsed ticks, not assumed callback frequency, and can be disabled under load.

## 9. Rendering and presentation contract

The first correct version may redraw the full frame. The order is deterministic and the cursor is composed last. Ensure source-over alpha uses the project's actual channel convention consistently. Alpha panels blend over wallpaper/windows; do not advertise blur until a sampling blur pass exists.

Mark dirty when wallpaper/layout changes, a window moves/resizes/changes content/state, a widget changes, or cursor moves. Once partial rendering is proven, union/clamp dirty rectangles and repaint background through cursor in each damaged area. If dirty coverage exceeds a measured threshold, fall back to full redraw. Current `DamageTracker` is only a helper and must not be described as an optimization until wired end to end.

Use an off-screen surface if the available memory budget permits, then copy rows using framebuffer pitch and pixel format. Handle 24/32-bit or other formats only when explicitly supported; otherwise fail cleanly and retain shell diagnostics. Never assume `pitch == width * bytes_per_pixel`. Check framebuffer address ranges and multiplication overflow before indexing. Hardware page flips and VSync are optional future capabilities and must be reported as such unless actually detected and used. The 8K JPEG master is a source asset; runtime uses the compiled RAW wallpaper, not an 8K decode in the hot render path.

## 10. Desktop session, launcher, and applications

Session startup sequence:

1. Finish boot display discovery and validate framebuffer metadata.
2. If display is invalid/unavailable, keep boot and serial/text shell working and defer graphics.
3. Initialize compositor surfaces and load embedded wallpaper; on wallpaper failure use a solid palette background.
4. Load a validated desktop configuration or use documented defaults.
5. Create `DesktopSession`, Aegis Bar, and dock; start with no demo windows.
6. Register available built-in applications and set their dock entries available only when they can launch.
7. Route post-start keyboard/pointer events through the desktop event queue; keep serial diagnostics working.
8. Render the first frame and then render on invalidation/scheduled animation.

The launcher opens from the P control and lists registered applications, not filenames discovered and blindly executed. Initially it can launch only built-in programs. `/System/Applications` is the proposed future installation location, not an existing executable ABI. `/System/Resources` is the intended shared-resource location; current embedded assets under `assets/` are compile-time project inputs. Define any on-disk package layout and trust rules separately.

Keep the current app boundary honest: no APApplication parser, permissions, GUI syscall, or dynamic loader exists yet. First provide kernel-owned surfaces for built-in clients. Any future user-mode protocol must validate every request and use handles/capabilities for windows, buffers, and events; never accept raw physical addresses or let clients map the screen framebuffer. Specify process lifecycle and IPC before advertising third-party apps.

## 11. Configuration and assets

The seeded `desktop.cfg` and `compositor.conf` are currently inert. Before consuming them, define a small versioned key/value format with bounded line/key/value lengths, comments, explicit defaults, and rejection of unknown unsafe values. Configuration may select theme tokens, wallpaper identifier, dock entries, and conservative compositor options. It must not choose arbitrary memory addresses, display modes, or executable paths. Missing/malformed configuration falls back to compiled defaults and logs one concise diagnostic.

Keep the 8K wallpaper master at `assets/wallpapers/purple_nebula_8k.jpg` and the runtime RAW asset at `assets/wallpapers/purple_nebula.pwraw`. Preserve source dimensions in the RAW header and validate them before allocation. Any new launcher/dock/window icon must be newly drawn or have recorded provenance/license. The current built-in font is bitmap `FONT8X16` with ASCII-oriented coverage; do not promise anti-aliased typography or full Unicode until a suitable font asset, renderer, and license are added. Gracefully render unknown glyphs.

## 12. Proposed PurpleOS interface shape

These are design names for new APIs, not existing symbols. Adapt to project conventions and keep interfaces small:

```rust
struct DesktopSession { /* bounded model, widgets, focus, event queue */ }
struct WindowId(u16);
enum DesktopEvent { PointerMove { x: i32, y: i32 }, Button { /* ... */ }, Key { /* ... */ } }
enum WindowState { Normal, Maximized, Minimized, Closing }

impl DesktopSession {
    fn initialize(display: DisplayInfo, config: DesktopConfig) -> Result<Self, DesktopError>;
    fn enqueue_input(&mut self, event: DesktopEvent) -> QueueResult;
    fn process_pending_events(&mut self, now: Tick) -> DesktopWork;
    fn request_window(&mut self, spec: WindowSpec) -> Result<WindowId, DesktopError>;
    fn render_if_dirty(&mut self, compositor: &mut CompositorEngine) -> Result<(), DisplayError>;
}
```

Use actual project error and ownership types when implementing. Keep queue insertion callable from interrupt-safe context only if its synchronization is proven safe. Public window operations validate all dimensions and handles. Avoid exposing mutable scene internals to applications.

## 13. Ordered implementation milestones

Complete in sequence so each stage leaves a bootable system:

1. **Correctness inventory:** confirm actual boot framebuffer formats, mouse/keyboard call paths, shell routing, lock use, and memory constraints. Update docs if implementation has changed.
2. **Safe event path:** define bounded input events/queue; change mouse and keyboard producers to enqueue; drain in non-interrupt context; preserve serial shell behavior.
3. **Window manager completion:** add resize capture, robust geometry constraints, input dispatch to built-in clients, focus restoration, close lifecycle, and clipping.
4. **Desktop session model:** introduce session ownership, shell widget state, app registry, task indicators, and explicit no-display fallback.
5. **Shell widgets:** render responsive Aegis Bar, dock, launcher, and transient overlay in the agreed layer order; use honest unavailable status.
6. **Frame policy:** dirty tracking, cursor correctness, full-redraw fallback, optional timer-driven animation capped by measured tick policy; document presentation capability.
7. **Configuration:** parse versioned settings with bounded input and defaults; connect seeded files or remove misleading unused knobs.
8. **Built-in apps:** add one built-in client at a time through the kernel-owned window/event contract. Treat user-mode app loading as a separate design and security milestone.
9. **Hardware/layout validation:** exercise available resolutions and framebuffer formats in emulator and on target hardware; verify no fixed mode assumptions and no out-of-bounds drawing.

## 14. Acceptance criteria and verification plan

The desktop foundation is complete only when all applicable criteria below are demonstrably met:

- Boot with a valid framebuffer at multiple aspect ratios/resolutions and show the wallpaper, bar, and dock with no clipping or fixed-resolution assumptions.
- Boot with no valid framebuffer and retain serial/text diagnostics without a graphics panic or invented fallback dimensions.
- Bar clock/status reflects real available data; absent subsystems are not presented as working.
- Dock distinguishes registered, unavailable, running, and minimized applications; restore/focus behavior is correct.
- Create, focus, move, resize, minimize, maximize/restore, close, and destroy windows. Keep titlebar reachable and maintain correct z-order.
- Route pointer and keyboard events to the correct target. Input IRQ code never renders, allocates, blocks, or calls clients. Queue overflow is observable and does not reorder/drop button or key transitions silently.
- Alpha, clipping, wallpaper scaling, cursor movement, and framebuffer pitch/pixel-format handling are validated. Full redraw remains a safe fallback.
- Configuration parsing handles missing, malformed, overlong, and unsupported settings using safe defaults.
- No source, resource, API, or generated visual was copied from the LG3D tree. Provenance/license records exist for added assets.
- Docs, journals, seeded configs, and status output distinguish implemented behavior from planned behavior.

Add or update focused tests as code is implemented: geometry/scale and clipping unit tests; window transition and hit-test tests; queue ordering/overflow tests; config parser tests; wallpaper header/scaling tests; alpha/pixel conversion tests; boot tests for absent/invalid framebuffer; emulator interaction checks for drag, resize, focus, dock restore, and shell/serial input. This document only specifies future verification; no tests were run or are implied by its authorship.

## 15. Related project documents

- `docs/PURPLE_DESKTOP_UI_SPECIFICATION.md` — approved visual direction and design tokens; some technical claims are aspirational and this blueprint corrects their implementation status.
- `docs/APAPPLICATION_SPECIFICATION.md` — proposed future application package contract; not implemented.
- `docs/PURPLE_OS_DIRECTORY_STRUCTURE_SPECIFICATION.md` — intended filesystem organization.
- `docs/PURPLE_OS_ARCHITECTURE_DECISIONS.md` — architecture decisions and constraints.
- `Journals/18-11-2026 - Native Framebuffer Compositor, Window Server Subsystem, and Resigned OS Hierarchy.md` — compositor history; verify claims against code.
- `Journals/18-11-2026 - Direct Native Compositor Boot Architecture, Display-Serial Decoupling, and CUI Preservation.md` — boot/input intent; reconcile shell input claims with current code.
- `Journals/18-11-2026 - Purple Desktop UI Concept, Looking Glass Modernization, and Final Specification.md` — visual design iteration; its Classpath Exception statement is superseded by the licensing review in this blueprint.
