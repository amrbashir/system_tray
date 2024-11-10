// Copyright 2022-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#![allow(unused)]

use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    menu::{
        AboutMetadata, CheckMenuItem, IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        Submenu,
    },
    TrayIconBuilder, TrayIconEvent,
};

enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // set a tray event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        proxy.send_event(UserEvent::TrayIconEvent(event));
    }));

    // set a menu event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        proxy.send_event(UserEvent::MenuEvent(event));
    }));

    let tray_menu = Menu::new();

    let icon_i = IconMenuItem::new(
        "Icon",
        true,
        Some(load_menu_icon(std::path::Path::new(path))),
        None,
    );
    let check_i = CheckMenuItem::new("Check", true, false, None);
    let subitem_i = MenuItem::new("Subitem", true, None);
    let submenu_i = Submenu::new("Submenu", true);
    submenu_i.append(&subitem_i);
    let quit_i = MenuItem::new("Quit", true, None);
    tray_menu.append_items(&[
        &PredefinedMenuItem::about(
            None,
            Some(AboutMetadata {
                name: Some("tao".to_string()),
                copyright: Some("Copyright tao".to_string()),
                ..Default::default()
            }),
        ),
        &PredefinedMenuItem::separator(),
        &icon_i,
        &check_i,
        &submenu_i,
        &quit_i,
    ]);

    let mut tray_icon = None;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
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

            Event::UserEvent(UserEvent::TrayIconEvent(event)) => {
                println!("{event:?}");
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                println!("{event:?}");

                if event.id == quit_i.id() {
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => {}
        }
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
