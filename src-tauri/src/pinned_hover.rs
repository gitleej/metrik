use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::WebviewWindow;

#[cfg(target_os = "linux")]
const HOVER_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(8);

/// One long-lived X11 hover monitor per application process.
///
/// The worker owns a private X connection and performs every pointer, opacity,
/// and input-shape operation on that connection. It must not call GTK/GDK:
/// Tauri's Linux dispatcher can execute callbacks on different Rust workers,
/// while GDK's X11 error-trap stack is thread-affine.
#[derive(Default)]
pub struct Monitor {
    enabled: AtomicBool,
    target_opacity_bits: AtomicU64,
    started: AtomicBool,
    available: AtomicBool,
}

pub fn configure(
    window: WebviewWindow,
    monitor: Arc<Monitor>,
    enabled: bool,
    target_opacity: f64,
) -> bool {
    #[cfg(target_os = "linux")]
    {
        if !super::linux_supports_global_window_coordinates() {
            monitor.enabled.store(false, Ordering::Release);
            return false;
        }
        monitor
            .target_opacity_bits
            .store(target_opacity.clamp(0.0, 1.0).to_bits(), Ordering::Release);

        if !enabled {
            // The monitor observes this within one polling interval and restores
            // both opacity and the default input shape on its own X connection.
            monitor.enabled.store(false, Ordering::Release);
            return false;
        }
        if !ensure_monitor(&window, Arc::clone(&monitor)) {
            monitor.enabled.store(false, Ordering::Release);
            return false;
        }
        monitor.enabled.store(true, Ordering::Release);
        monitor.available.load(Ordering::Acquire)
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, monitor, enabled, target_opacity);
        false
    }
}

#[cfg(target_os = "linux")]
fn ensure_monitor(window: &WebviewWindow, monitor: Arc<Monitor>) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if monitor.available.load(Ordering::Acquire) {
        return true;
    }
    if monitor
        .started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return monitor.available.load(Ordering::Acquire);
    }

    let xid = match window.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xlib(handle)) if handle.window != 0 => handle.window,
        _ => {
            monitor.started.store(false, Ordering::Release);
            return false;
        }
    };
    let Some(native) = X11Window::connect(xid) else {
        monitor.started.store(false, Ordering::Release);
        return false;
    };

    let thread_monitor = Arc::clone(&monitor);
    let spawned = std::thread::Builder::new()
        .name("metrik-pinned-hover".into())
        .spawn(move || run_hover_monitor(native, thread_monitor))
        .is_ok();
    if spawned {
        monitor.available.store(true, Ordering::Release);
    } else {
        monitor.started.store(false, Ordering::Release);
    }
    spawned
}

#[cfg(target_os = "linux")]
fn run_hover_monitor(mut native: X11Window, monitor: Arc<Monitor>) {
    let mut last_inside = false;
    let mut last_target_bits = 1.0_f64.to_bits();

    loop {
        if monitor.enabled.load(Ordering::Acquire) {
            let target_bits = monitor.target_opacity_bits.load(Ordering::Acquire);
            let inside = native.pointer_inside().unwrap_or(last_inside);
            if inside != last_inside || (inside && target_bits != last_target_bits) {
                native.apply_hover_state(inside, f64::from_bits(target_bits));
                last_inside = inside;
                last_target_bits = target_bits;
            }
        } else if last_inside {
            native.apply_hover_state(false, 1.0);
            last_inside = false;
            last_target_bits = 1.0_f64.to_bits();
        }
        std::thread::sleep(HOVER_POLL_INTERVAL);
    }
}

#[cfg(target_os = "linux")]
fn hover_state(inside: bool, target_opacity: f64) -> (f64, bool) {
    let opacity = if inside {
        target_opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    // Fade and complete-hide are passive presentation modes. In either case,
    // clicks must reach the desktop underneath while the pointer is inside.
    (opacity, inside)
}

#[cfg(target_os = "linux")]
fn point_inside_window(pointer_x: i32, pointer_y: i32, width: u32, height: u32) -> bool {
    pointer_x >= 0
        && pointer_y >= 0
        && i64::from(pointer_x) < i64::from(width)
        && i64::from(pointer_y) < i64::from(height)
}

#[cfg(target_os = "linux")]
struct X11Window {
    display: *mut xlib::Display,
    window: xlib::Window,
    opacity_atom: xlib::Atom,
}

// The connection is moved once into the monitor and then exclusively used and
// closed by that thread. No Xlib object from GTK/GDK is shared with it.
#[cfg(target_os = "linux")]
unsafe impl Send for X11Window {}

#[cfg(target_os = "linux")]
impl X11Window {
    fn connect(window: xlib::Window) -> Option<Self> {
        let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
        if display.is_null() {
            return None;
        }

        let mut event_base = 0;
        let mut error_base = 0;
        let shape_available =
            unsafe { xlib::XShapeQueryExtension(display, &mut event_base, &mut error_base) != 0 };
        if !shape_available {
            unsafe { xlib::XCloseDisplay(display) };
            return None;
        }

        let opacity_atom =
            unsafe { xlib::XInternAtom(display, c"_NET_WM_WINDOW_OPACITY".as_ptr(), xlib::FALSE) };
        Some(Self {
            display,
            window,
            opacity_atom,
        })
    }

    fn pointer_inside(&self) -> Option<bool> {
        let mut root_return = 0;
        let mut child_return = 0;
        let mut root_x = 0;
        let mut root_y = 0;
        let mut window_x = 0;
        let mut window_y = 0;
        let mut mask = 0;
        let success = unsafe {
            xlib::XQueryPointer(
                self.display,
                self.window,
                &mut root_return,
                &mut child_return,
                &mut root_x,
                &mut root_y,
                &mut window_x,
                &mut window_y,
                &mut mask,
            )
        };
        if success == 0 {
            return None;
        }

        let mut geometry_root = 0;
        let mut x = 0;
        let mut y = 0;
        let mut width = 0;
        let mut height = 0;
        let mut border_width = 0;
        let mut depth = 0;
        let geometry_success = unsafe {
            xlib::XGetGeometry(
                self.display,
                self.window,
                &mut geometry_root,
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                &mut border_width,
                &mut depth,
            )
        };
        (geometry_success != 0).then_some(point_inside_window(window_x, window_y, width, height))
    }

    fn apply_hover_state(&mut self, inside: bool, target_opacity: f64) {
        let (opacity, ignore_cursor_events) = hover_state(inside, target_opacity);
        unsafe {
            if opacity >= 1.0 {
                xlib::XDeleteProperty(self.display, self.window, self.opacity_atom);
            } else {
                let opacity_value = (opacity * f64::from(u32::MAX)).round() as u64;
                xlib::XChangeProperty(
                    self.display,
                    self.window,
                    self.opacity_atom,
                    xlib::XA_CARDINAL,
                    32,
                    xlib::PROP_MODE_REPLACE,
                    (&opacity_value as *const u64).cast(),
                    1,
                );
            }

            if ignore_cursor_events {
                xlib::XShapeCombineRectangles(
                    self.display,
                    self.window,
                    xlib::SHAPE_INPUT,
                    0,
                    0,
                    std::ptr::null_mut(),
                    0,
                    xlib::SHAPE_SET,
                    xlib::UNSORTED,
                );
            } else {
                xlib::XShapeCombineMask(
                    self.display,
                    self.window,
                    xlib::SHAPE_INPUT,
                    0,
                    0,
                    xlib::NONE,
                    xlib::SHAPE_SET,
                );
            }
            xlib::XFlush(self.display);
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for X11Window {
    fn drop(&mut self) {
        self.apply_hover_state(false, 1.0);
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}

#[cfg(target_os = "linux")]
mod xlib {
    use std::ffi::{c_char, c_int, c_uchar, c_uint, c_ulong};

    #[repr(C)]
    pub struct Display {
        _private: [u8; 0],
    }

    pub type Window = c_ulong;
    pub type Atom = c_ulong;
    pub type Pixmap = c_ulong;

    pub const FALSE: c_int = 0;
    pub const NONE: Pixmap = 0;
    pub const XA_CARDINAL: Atom = 6;
    pub const PROP_MODE_REPLACE: c_int = 0;
    pub const SHAPE_INPUT: c_int = 2;
    pub const SHAPE_SET: c_int = 0;
    pub const UNSORTED: c_int = 0;

    #[repr(C)]
    pub struct XRectangle {
        pub x: i16,
        pub y: i16,
        pub width: u16,
        pub height: u16,
    }

    #[link(name = "X11")]
    extern "C" {
        pub fn XOpenDisplay(display_name: *const c_char) -> *mut Display;
        pub fn XInternAtom(
            display: *mut Display,
            name: *const c_char,
            only_if_exists: c_int,
        ) -> Atom;
        pub fn XQueryPointer(
            display: *mut Display,
            window: Window,
            root_return: *mut Window,
            child_return: *mut Window,
            root_x_return: *mut c_int,
            root_y_return: *mut c_int,
            window_x_return: *mut c_int,
            window_y_return: *mut c_int,
            mask_return: *mut c_uint,
        ) -> c_int;
        pub fn XGetGeometry(
            display: *mut Display,
            drawable: Window,
            root_return: *mut Window,
            x_return: *mut c_int,
            y_return: *mut c_int,
            width_return: *mut c_uint,
            height_return: *mut c_uint,
            border_width_return: *mut c_uint,
            depth_return: *mut c_uint,
        ) -> c_int;
        pub fn XChangeProperty(
            display: *mut Display,
            window: Window,
            property: Atom,
            property_type: Atom,
            format: c_int,
            mode: c_int,
            data: *const c_uchar,
            element_count: c_int,
        ) -> c_int;
        pub fn XDeleteProperty(display: *mut Display, window: Window, property: Atom) -> c_int;
        pub fn XFlush(display: *mut Display) -> c_int;
        pub fn XCloseDisplay(display: *mut Display) -> c_int;
    }

    #[link(name = "Xext")]
    extern "C" {
        pub fn XShapeQueryExtension(
            display: *mut Display,
            event_base_return: *mut c_int,
            error_base_return: *mut c_int,
        ) -> c_int;
        pub fn XShapeCombineRectangles(
            display: *mut Display,
            window: Window,
            dest_kind: c_int,
            x_offset: c_int,
            y_offset: c_int,
            rectangles: *mut XRectangle,
            rectangle_count: c_int,
            operation: c_int,
            ordering: c_int,
        );
        pub fn XShapeCombineMask(
            display: *mut Display,
            window: Window,
            dest_kind: c_int,
            x_offset: c_int,
            y_offset: c_int,
            source: Pixmap,
            operation: c_int,
        );
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{hover_state, point_inside_window};

    #[test]
    fn window_hit_test_uses_half_open_relative_bounds() {
        assert!(point_inside_window(0, 0, 320, 28));
        assert!(point_inside_window(319, 27, 320, 28));
        assert!(!point_inside_window(320, 27, 320, 28));
        assert!(!point_inside_window(319, 28, 320, 28));
    }

    #[test]
    fn negative_relative_coordinates_are_outside() {
        assert!(!point_inside_window(-1, 0, 320, 320));
        assert!(!point_inside_window(0, -1, 320, 320));
    }

    #[test]
    fn complete_hide_disables_native_cursor_hit_testing() {
        assert_eq!(hover_state(true, 0.0), (0.0, true));
    }

    #[test]
    fn fade_disables_native_cursor_hit_testing() {
        assert_eq!(hover_state(true, 0.35), (0.35, true));
    }

    #[test]
    fn pointer_exit_restores_visibility_and_hit_testing() {
        assert_eq!(hover_state(false, 0.0), (1.0, false));
    }
}
