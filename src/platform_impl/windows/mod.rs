// Copyright 2022-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod icon;
mod util;
use std::ptr;

use once_cell::sync::Lazy;
use windows_sys::{
    s,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        UI::{
            Shell::{
                DefSubclassProc, SetWindowSubclass, Shell_NotifyIconGetRect, Shell_NotifyIconW,
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
                NOTIFYICONIDENTIFIER,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, RegisterClassW,
                RegisterWindowMessageA, SendMessageW, SetForegroundWindow, TrackPopupMenu,
                CW_USEDEFAULT, HICON, HMENU, TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_DESTROY,
                WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW, WS_EX_LAYERED,
                WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED,
            },
        },
    },
};

use crate::{
    icon::Icon, menu, ClickType, Rect, TrayIconAttributes, TrayIconEvent, TrayIconId, COUNTER,
};

pub(crate) use self::icon::WinIcon as PlatformIcon;

const TRAY_SUBCLASS_ID: usize = 6001;
const WM_USER_TRAYICON: u32 = 6002;
const WM_USER_UPDATE_TRAYMENU: u32 = 6003;
const WM_USER_UPDATE_TRAYICON: u32 = 6004;
const WM_USER_SHOW_TRAYICON: u32 = 6005;
const WM_USER_HIDE_TRAYICON: u32 = 6006;
const WM_USER_UPDATE_TRAYTOOLTIP: u32 = 6007;

/// When the taskbar is created, it registers a message with the "TaskbarCreated" string and then broadcasts this message to all top-level windows
/// When the application receives this message, it should assume that any taskbar icons it added have been removed and add them again.
static S_U_TASKBAR_RESTART: Lazy<u32> =
    Lazy::new(|| unsafe { RegisterWindowMessageA(s!("TaskbarCreated")) });

struct TrayLoopData {
    internal_id: u32,
    id: TrayIconId,
    hwnd: HWND,
    hpopupmenu: Option<HMENU>,
    icon: Option<Icon>,
    tooltip: Option<String>,
}

pub struct TrayIcon {
    hwnd: HWND,
    menu: Option<Box<dyn menu::ContextMenu>>,
    internal_id: u32,
}

impl TrayIcon {
    pub fn new(id: TrayIconId, attrs: TrayIconAttributes) -> crate::Result<Self> {
        let internal_id = COUNTER.next();

        let class_name = util::encode_wide("tray_icon_app");
        unsafe {
            let hinstance = util::get_instance_handle();

            unsafe extern "system" fn call_default_window_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(call_default_window_proc),
                lpszClassName: class_name.as_ptr(),
                hInstance: hinstance,
                ..std::mem::zeroed()
            };

            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED |
            // WS_EX_TOOLWINDOW prevents this window from ever showing up in the taskbar, which
            // we want to avoid. If you remove this style, this window won't show up in the
            // taskbar *initially*, but it can show up at some later point. This can sometimes
            // happen on its own after several hours have passed, although this has proven
            // difficult to reproduce. Alternatively, it can be manually triggered by killing
            // `explorer.exe` and then starting the process back up.
            // It is unclear why the bug is triggered by waiting for several hours.
            WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                ptr::null(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                0,
                CW_USEDEFAULT,
                0,
                HWND::default(),
                HMENU::default(),
                hinstance,
                std::ptr::null_mut(),
            );
            if hwnd == 0 {
                return Err(crate::Error::OsError(std::io::Error::last_os_error()));
            }

            let hicon = attrs.icon.as_ref().map(|i| i.inner.as_raw_handle());

            if !register_tray_icon(hwnd, internal_id, &hicon, &attrs.tooltip) {
                return Err(crate::Error::OsError(std::io::Error::last_os_error()));
            }

            if let Some(menu) = &attrs.menu {
                menu.attach_menu_subclass_for_hwnd(hwnd);
            }

            // tray-icon event handler
            let traydata = TrayLoopData {
                id,
                internal_id,
                hwnd,
                hpopupmenu: attrs.menu.as_ref().map(|m| m.hpopupmenu()),
                icon: attrs.icon,
                tooltip: attrs.tooltip,
            };
            SetWindowSubclass(
                hwnd,
                Some(tray_subclass_proc),
                TRAY_SUBCLASS_ID,
                Box::into_raw(Box::new(traydata)) as _,
            );

            Ok(Self {
                hwnd,
                internal_id,
                menu: attrs.menu,
            })
        }
    }

    pub fn set_icon(&mut self, icon: Option<Icon>) -> crate::Result<()> {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                uFlags: NIF_ICON,
                hWnd: self.hwnd,
                uID: self.internal_id,
                ..std::mem::zeroed()
            };

            if let Some(hicon) = icon.as_ref().map(|i| i.inner.as_raw_handle()) {
                nid.hIcon = hicon;
            }

            if Shell_NotifyIconW(NIM_MODIFY, &mut nid as _) == 0 {
                return Err(crate::Error::OsError(std::io::Error::last_os_error()));
            }

            // send the new icon to the subclass proc to store it in the tray data
            SendMessageW(
                self.hwnd,
                WM_USER_UPDATE_TRAYICON,
                Box::into_raw(Box::new(icon)) as _,
                0,
            );
        }

        Ok(())
    }

    pub fn set_menu(&mut self, menu: Option<Box<dyn menu::ContextMenu>>) {
        if let Some(menu) = &self.menu {
            menu.detach_menu_subclass_from_hwnd(self.hwnd);
        }

        if let Some(menu) = &menu {
            menu.attach_menu_subclass_for_hwnd(self.hwnd);
        }

        unsafe {
            // send the new menu to the subclass proc where we will update there
            SendMessageW(
                self.hwnd,
                WM_USER_UPDATE_TRAYMENU,
                Box::into_raw(Box::new(menu.as_ref().map(|m| m.hpopupmenu()))) as _,
                0,
            );
        }

        self.menu = menu;
    }

    pub fn set_tooltip<S: AsRef<str>>(&mut self, tooltip: Option<S>) -> crate::Result<()> {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                uFlags: NIF_TIP,
                hWnd: self.hwnd,
                uID: self.internal_id,
                ..std::mem::zeroed()
            };
            if let Some(tooltip) = &tooltip {
                let tip = util::encode_wide(tooltip.as_ref());
                #[allow(clippy::manual_memcpy)]
                for i in 0..tip.len().min(128) {
                    nid.szTip[i] = tip[i];
                }
            }

            if Shell_NotifyIconW(NIM_MODIFY, &mut nid as _) == 0 {
                return Err(crate::Error::OsError(std::io::Error::last_os_error()));
            }

            // send the new tooltip to the subclass proc to store it in the tray data
            SendMessageW(
                self.hwnd,
                WM_USER_UPDATE_TRAYTOOLTIP,
                Box::into_raw(Box::new(tooltip.map(|t| t.as_ref().to_string()))) as _,
                0,
            );
        }

        Ok(())
    }

    pub fn set_title<S: AsRef<str>>(&mut self, _title: Option<S>) {}

    pub fn set_visible(&mut self, visible: bool) -> crate::Result<()> {
        unsafe {
            SendMessageW(
                self.hwnd,
                if visible {
                    WM_USER_SHOW_TRAYICON
                } else {
                    WM_USER_HIDE_TRAYICON
                },
                0,
                0,
            );
        }

        Ok(())
    }

    pub fn rect(&self) -> Option<Rect> {
        let dpi = unsafe { util::hwnd_dpi(self.hwnd) };
        let scale_factor = util::dpi_to_scale_factor(dpi);
        Some(get_tray_rect(self.internal_id, self.hwnd, scale_factor))
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            remove_tray_icon(self.hwnd, self.internal_id);

            if let Some(menu) = &self.menu {
                menu.detach_menu_subclass_from_hwnd(self.hwnd);
            }

            // destroy the hidden window used by the tray
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn tray_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    subclass_input_ptr: usize,
) -> LRESULT {
    let subclass_input_ptr = subclass_input_ptr as *mut TrayLoopData;
    let subclass_input = &mut *(subclass_input_ptr);

    match msg {
        WM_DESTROY => {
            drop(Box::from_raw(subclass_input_ptr));
            return 0;
        }
        WM_USER_UPDATE_TRAYMENU => {
            let hpopupmenu = Box::from_raw(wparam as *mut Option<isize>);
            subclass_input.hpopupmenu = *hpopupmenu;
        }
        WM_USER_UPDATE_TRAYICON => {
            let icon = Box::from_raw(wparam as *mut Option<Icon>);
            subclass_input.icon = *icon;
        }
        WM_USER_SHOW_TRAYICON => {
            register_tray_icon(
                subclass_input.hwnd,
                subclass_input.internal_id,
                &subclass_input
                    .icon
                    .as_ref()
                    .map(|i| i.inner.as_raw_handle()),
                &subclass_input.tooltip,
            );
        }
        WM_USER_HIDE_TRAYICON => {
            remove_tray_icon(subclass_input.hwnd, subclass_input.internal_id);
        }
        WM_USER_UPDATE_TRAYTOOLTIP => {
            let tooltip = Box::from_raw(wparam as *mut Option<String>);
            subclass_input.tooltip = *tooltip;
        }
        _ if msg == *S_U_TASKBAR_RESTART => {
            register_tray_icon(
                subclass_input.hwnd,
                subclass_input.internal_id,
                &subclass_input
                    .icon
                    .as_ref()
                    .map(|i| i.inner.as_raw_handle()),
                &subclass_input.tooltip,
            );
        }
        WM_USER_TRAYICON
            if matches!(
                lparam as u32,
                WM_LBUTTONUP | WM_RBUTTONUP | WM_LBUTTONDBLCLK
            ) =>
        {
            let mut cursor = POINT { x: 0, y: 0 };
            GetCursorPos(&mut cursor as _);

            let x = cursor.x as f64;
            let y = cursor.y as f64;

            let event = match lparam as u32 {
                WM_LBUTTONUP => ClickType::Left,
                WM_RBUTTONUP => ClickType::Right,
                WM_LBUTTONDBLCLK => ClickType::Double,
                _ => unreachable!(),
            };

            let dpi = util::hwnd_dpi(hwnd);
            let scale_factor = util::dpi_to_scale_factor(dpi);

            TrayIconEvent::send(crate::TrayIconEvent {
                id: subclass_input.id.clone(),
                position: crate::dpi::LogicalPosition::new(x, y).to_physical(scale_factor),
                icon_rect: get_tray_rect(subclass_input.internal_id, hwnd, scale_factor),
                click_type: event,
            });

            if lparam as u32 == WM_RBUTTONUP {
                if let Some(menu) = subclass_input.hpopupmenu {
                    show_tray_menu(hwnd, menu, cursor.x, cursor.y);
                }
            }
        }
        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[inline]
unsafe fn show_tray_menu(hwnd: HWND, menu: HMENU, x: i32, y: i32) {
    // bring the hidden window to the foreground so the pop up menu
    // would automatically hide on click outside
    SetForegroundWindow(hwnd);
    TrackPopupMenu(
        menu,
        // align bottom / right, maybe we could expose this later..
        TPM_BOTTOMALIGN | TPM_LEFTALIGN,
        x,
        y,
        0,
        hwnd,
        std::ptr::null_mut(),
    );
}

#[inline]
unsafe fn register_tray_icon(
    hwnd: HWND,
    tray_id: u32,
    hicon: &Option<HICON>,
    tooltip: &Option<String>,
) -> bool {
    let mut h_icon = 0;
    let mut flags = NIF_MESSAGE;
    let mut sz_tip: [u16; 128] = [0; 128];

    if let Some(hicon) = hicon {
        flags |= NIF_ICON;
        h_icon = *hicon;
    }

    if let Some(tooltip) = tooltip {
        flags |= NIF_TIP;
        let tip = util::encode_wide(tooltip);
        #[allow(clippy::manual_memcpy)]
        for i in 0..tip.len().min(128) {
            sz_tip[i] = tip[i];
        }
    }

    let mut nid = NOTIFYICONDATAW {
        uFlags: flags,
        hWnd: hwnd,
        uID: tray_id,
        uCallbackMessage: WM_USER_TRAYICON,
        hIcon: h_icon,
        szTip: sz_tip,
        ..std::mem::zeroed()
    };

    Shell_NotifyIconW(NIM_ADD, &mut nid as _) == 1
}

#[inline]
unsafe fn remove_tray_icon(hwnd: HWND, id: u32) {
    let mut nid = NOTIFYICONDATAW {
        uFlags: NIF_ICON,
        hWnd: hwnd,
        uID: id,
        ..std::mem::zeroed()
    };

    if Shell_NotifyIconW(NIM_DELETE, &mut nid as _) == 0 {
        eprintln!("Error removing system tray icon");
    }
}

#[inline]
fn get_tray_rect(id: u32, hwnd: HWND, scale_factor: f64) -> Rect {
    let nid = NOTIFYICONIDENTIFIER {
        hWnd: hwnd,
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as _,
        uID: id,
        ..unsafe { std::mem::zeroed() }
    };

    let mut icon_rect = RECT {
        left: 0,
        bottom: 0,
        right: 0,
        top: 0,
    };
    unsafe { Shell_NotifyIconGetRect(&nid, &mut icon_rect) };

    Rect {
        position: crate::dpi::LogicalPosition::new(icon_rect.left, icon_rect.top)
            .to_physical(scale_factor),
        size: crate::dpi::LogicalSize::new(
            icon_rect.right.saturating_sub(icon_rect.left),
            icon_rect.bottom.saturating_sub(icon_rect.top),
        )
        .to_physical(scale_factor),
    }
}
