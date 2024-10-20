// Copyright 2022-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod icon;
mod menu;
mod tray;

pub(crate) use icon::PlatformIcon;
use tray::Tray;

use crate::{icon::Icon, TrayIconAttributes, TrayIconId};

pub struct TrayIcon {
    tray_handle: ksni::Handle<Tray>,
}

impl TrayIcon {
    pub fn new(id: TrayIconId, attrs: TrayIconAttributes) -> crate::Result<Self> {
        let icon = attrs.icon.map(|icon| icon.inner.into());

        let title = title_or_pkg_name(attrs.title.unwrap_or_default());

        let tooltip = attrs.tooltip.unwrap_or_default();

        let menu = attrs
            .menu
            .as_ref()
            .map(|menu| menu.items().into_iter().map(Into::into).collect())
            .unwrap_or_default();

        let tray_service = ksni::TrayService::new(Tray::new(id, icon, title, tooltip, menu));
        let tray_handle = tray_service.handle();
        tray_service.spawn();

        Ok(Self { tray_handle })
    }

    pub fn set_icon(&mut self, icon: Option<Icon>) -> crate::Result<()> {
        let icon = icon.map(|icon| icon.inner.into());

        self.tray_handle.update(|tray| {
            tray.set_icon(icon);
        });

        Ok(())
    }

    pub fn set_menu(&mut self, menu: Option<Box<dyn crate::menu::ContextMenu>>) {
        let menu = menu
            .as_ref()
            .map(|menu| menu.items().into_iter().map(Into::into).collect())
            .unwrap_or_default();

        self.tray_handle.update(|tray| {
            tray.set_menu(menu);
        });
    }

    pub fn set_tooltip<S: AsRef<str>>(&mut self, tooltip: Option<S>) -> crate::Result<()> {
        let tooltip = tooltip
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or_default()
            .to_string();

        self.tray_handle.update(|tray| {
            tray.set_tooltip(tooltip);
        });

        Ok(())
    }

    pub fn set_title<S: AsRef<str>>(&mut self, title: Option<S>) {
        let title = title
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or_default()
            .to_string();
        let title = title_or_pkg_name(title);

        self.tray_handle.update(|tray| {
            tray.set_title(title);
        });
    }

    pub fn set_visible(&mut self, visible: bool) -> crate::Result<()> {
        self.tray_handle.update(|tray| {
            if visible {
                tray.set_status(ksni::Status::Active);
            } else {
                tray.set_status(ksni::Status::Passive);
            }
        });

        Ok(())
    }

    pub fn rect(&self) -> Option<crate::Rect> {
        None
    }
}

fn title_or_pkg_name(title: String) -> String {
    if !title.is_empty() {
        title
    } else {
        env!("CARGO_PKG_NAME").into()
    }
}
