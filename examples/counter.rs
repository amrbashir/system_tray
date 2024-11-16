// Copyright 2022-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#![allow(unused)]

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{
        AboutMetadata, CheckMenuItem, IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        Submenu,
    },
    TrayIconBuilder, TrayIconEvent,
};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");

    let event_loop = EventLoopBuilder::new().build();

    let mut counter = 0;
    let tray_menu = Menu::new();

    let counter_i = MenuItem::new(format!("Counter: {counter}"), true, None);
    tray_menu.append_items(&[&counter_i]);

    let mut tray_icon = None;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    event_loop.run(move |event, _, control_flow| {
        // We add delay of 16 ms (60fps) to event_loop to reduce cpu load.
        // Alternatively, you can set ControlFlow::Wait or use TrayIconEvent::set_event_handler,
        // see https://github.com/tauri-apps/tray-icon/issues/83#issuecomment-1697773065
        *control_flow = ControlFlow::Poll;
        std::thread::sleep(std::time::Duration::from_millis(16));

        if let tao::event::Event::NewEvents(tao::event::StartCause::Init) = event {
            let icon = load_tray_icon(std::path::Path::new(path));

            // We create the icon once the event loop is actually running
            // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
            tray_icon = Some(
                TrayIconBuilder::new()
                    .with_menu(Box::new(tray_menu.clone()))
                    .with_tooltip("tao - awesome windowing lib")
                    .with_icon(icon)
                    .build()
                    .unwrap(),
            );

            // We have to request a redraw here to have the icon actually show up.
            // Tao only exposes a redraw method on the Window so we use core-foundation directly.
            #[cfg(target_os = "macos")]
            unsafe {
                use core_foundation::runloop::{CFRunLoopGetMain, CFRunLoopWakeUp};

                let rl = CFRunLoopGetMain();
                CFRunLoopWakeUp(rl);
            }
        }

        counter += 1;
        counter_i.set_text(format!("Counter: {counter}"));
    })
}

fn load_icon(path: &std::path::Path) -> (Vec<u8>, u32, u32) {
    let image = image::open(path)
        .expect("Failed to open icon path")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    (rgba, width, height)
}

fn load_tray_icon(path: &std::path::Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = load_icon(path);
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}

fn load_menu_icon(path: &std::path::Path) -> muda::Icon {
    let (icon_rgba, icon_width, icon_height) = load_icon(path);
    muda::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
