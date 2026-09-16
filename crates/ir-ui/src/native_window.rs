use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(windows)]
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowAction {
    BeginDrag,
    BeginResize(ResizeDirection),
    Minimize,
    ToggleMaximize,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResizeDirection {
    North,
    South,
    West,
    East,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

#[derive(Clone, Copy)]
struct NativeWindowHandle(isize);

const HANDLE_ID: &str = "ir_mixer_native_window_handle";
#[cfg(windows)]
const ICONS_ID: &str = "ir_mixer_native_window_icons";

#[cfg(windows)]
#[derive(Clone)]
struct NativeWindowIcons {
    _handles: Arc<NativeIconHandles>,
}

#[cfg(windows)]
struct NativeIconHandles {
    big: isize,
    small: isize,
}

#[cfg(windows)]
impl Drop for NativeIconHandles {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;
        unsafe {
            if self.big != 0 {
                DestroyIcon(self.big as *mut core::ffi::c_void);
            }
            if self.small != 0 {
                DestroyIcon(self.small as *mut core::ffi::c_void);
            }
        }
    }
}

/// Removes the operating-system title bar and remembers the native handle for
/// standalone window controls. Plugin editors never call this function.
pub fn configure_borderless(ctx: &egui::Context, window: &impl HasWindowHandle, title: &str) {
    #[cfg(windows)]
    if let Ok(handle) = window.window_handle()
        && let RawWindowHandle::Win32(handle) = handle.as_raw()
    {
        use windows_sys::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DwmSetWindowAttribute};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            SetWindowLongPtrW, SetWindowPos, SetWindowTextW, WS_CAPTION, WS_POPUP,
        };

        let hwnd = handle.hwnd.get() as *mut core::ffi::c_void;
        let native_title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        unsafe {
            SetWindowTextW(hwnd, native_title.as_ptr());
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            // Baseview creates standalone windows as popups. Popup maximization
            // covers the whole monitor, including the taskbar, so convert it to
            // an overlapped window while retaining its resize/min/max flags.
            // DWM border suppression below keeps the result visually borderless.
            SetWindowLongPtrW(hwnd, GWL_STYLE, style & !((WS_CAPTION | WS_POPUP) as isize));
            let no_border: u32 = 0xFFFF_FFFE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                (&no_border as *const u32).cast(),
                size_of::<u32>() as u32,
            );
            SetWindowPos(
                hwnd,
                core::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new(HANDLE_ID), NativeWindowHandle(hwnd as isize));
            if let Some(icons) = install_window_icons(hwnd) {
                data.insert_temp(egui::Id::new(ICONS_ID), icons);
            }
        });
    }
}

#[cfg(windows)]
fn install_window_icons(hwnd: *mut core::ffi::c_void) -> Option<NativeWindowIcons> {
    use image::ImageFormat;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIconFromResourceEx, GetSystemMetrics, ICON_BIG, ICON_SMALL, LR_DEFAULTCOLOR,
        SM_CXICON, SM_CXSMICON, SendMessageW, WM_SETICON,
    };

    let source = image::load_from_memory(include_bytes!("../../../design/logos/desktop-icon.png"))
        .ok()?
        .to_rgba8();

    let create_icon = |size: i32| {
        let scaled = image::imageops::resize(
            &source,
            size.max(1) as u32,
            size.max(1) as u32,
            image::imageops::FilterType::Lanczos3,
        );
        let mut png = Vec::new();
        scaled
            .write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
            .ok()?;
        let handle = unsafe {
            CreateIconFromResourceEx(
                png.as_ptr(),
                png.len() as u32,
                1,
                0x0003_0000,
                size,
                size,
                LR_DEFAULTCOLOR,
            )
        };
        (!handle.is_null()).then_some(handle as isize)
    };

    let big = create_icon(unsafe { GetSystemMetrics(SM_CXICON) })?;
    let small = match create_icon(unsafe { GetSystemMetrics(SM_CXSMICON) }) {
        Some(icon) => icon,
        None => {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(
                    big as *mut core::ffi::c_void,
                );
            }
            return None;
        }
    };
    unsafe {
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, big);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small);
    }
    Some(NativeWindowIcons {
        _handles: Arc::new(NativeIconHandles { big, small }),
    })
}

pub fn request(ctx: &egui::Context, action: NativeWindowAction) {
    #[cfg(windows)]
    if let Some(handle) =
        ctx.data(|data| data.get_temp::<NativeWindowHandle>(egui::Id::new(HANDLE_ID)))
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
            HTTOPRIGHT, IsZoomed, PostMessageW, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, WM_LBUTTONUP,
            WM_NCLBUTTONDOWN, WM_SYSCOMMAND,
        };

        let hwnd = handle.0 as *mut core::ffi::c_void;
        unsafe {
            match action {
                NativeWindowAction::BeginDrag => {
                    ReleaseCapture();
                    // The native modal move loop consumes the physical button-up
                    // event. Queue a client release first so baseview/egui does
                    // not retain a stuck primary-pointer state after the move.
                    PostMessageW(hwnd, WM_LBUTTONUP, 0, 0);
                    PostMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                }
                NativeWindowAction::BeginResize(direction) => {
                    let hit_test = match direction {
                        ResizeDirection::North => HTTOP,
                        ResizeDirection::South => HTBOTTOM,
                        ResizeDirection::West => HTLEFT,
                        ResizeDirection::East => HTRIGHT,
                        ResizeDirection::NorthWest => HTTOPLEFT,
                        ResizeDirection::NorthEast => HTTOPRIGHT,
                        ResizeDirection::SouthWest => HTBOTTOMLEFT,
                        ResizeDirection::SouthEast => HTBOTTOMRIGHT,
                    };
                    ReleaseCapture();
                    PostMessageW(hwnd, WM_LBUTTONUP, 0, 0);
                    PostMessageW(hwnd, WM_NCLBUTTONDOWN, hit_test as usize, 0);
                }
                NativeWindowAction::Minimize => {
                    PostMessageW(hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0);
                }
                NativeWindowAction::ToggleMaximize => {
                    PostMessageW(
                        hwnd,
                        WM_SYSCOMMAND,
                        if IsZoomed(hwnd) != 0 {
                            SC_RESTORE as usize
                        } else {
                            SC_MAXIMIZE as usize
                        },
                        0,
                    );
                }
                NativeWindowAction::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    #[cfg(not(windows))]
    if action == NativeWindowAction::Close {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// Adds invisible resize handles to a borderless standalone window.
pub fn show_resize_handles(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    let edge = 6.0;
    let corner = 12.0;
    let handles = [
        (
            ResizeDirection::West,
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + edge, rect.bottom())),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            ResizeDirection::East,
            egui::Rect::from_min_max(egui::pos2(rect.right() - edge, rect.top()), rect.max),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            ResizeDirection::North,
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + edge)),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            ResizeDirection::South,
            egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - edge), rect.max),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            ResizeDirection::NorthWest,
            egui::Rect::from_min_size(rect.min, egui::vec2(corner, corner)),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            ResizeDirection::NorthEast,
            egui::Rect::from_min_size(
                egui::pos2(rect.right() - corner, rect.top()),
                egui::vec2(corner, corner),
            ),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            ResizeDirection::SouthWest,
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.bottom() - corner),
                egui::vec2(corner, corner),
            ),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            ResizeDirection::SouthEast,
            egui::Rect::from_min_size(
                rect.max - egui::vec2(corner, corner),
                egui::vec2(corner, corner),
            ),
            egui::CursorIcon::ResizeNwSe,
        ),
    ];

    for (direction, handle_rect, cursor) in handles {
        let response = ui.interact(
            handle_rect,
            egui::Id::new(("native_resize", direction)),
            egui::Sense::drag(),
        );
        if response.hovered() || response.dragged() {
            ui.output_mut(|output| output.cursor_icon = cursor);
        }
        if response.drag_started() {
            request(ui.ctx(), NativeWindowAction::BeginResize(direction));
        }
    }
}
