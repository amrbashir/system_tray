use muda::{AboutDialog, PredefinedMenuItemType};

use super::tray::Tray;

#[derive(Debug, Clone)]
pub struct StandardItem {
    id: String,
    label: String,
    enabled: bool,
    icon: Option<Vec<u8>>,
    predefined_menu_item_type: Option<PredefinedMenuItemType>,
}

#[derive(Debug, Clone)]
pub struct CheckmarkItem {
    id: String,
    label: String,
    enabled: bool,
    checked: bool,
}

#[derive(Debug, Clone)]
pub struct SubMenuItem {
    label: String,
    enabled: bool,
    submenu: Vec<MenuItem>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum MenuItem {
    Standard(StandardItem),
    Checkmark(CheckmarkItem),
    SubMenu(SubMenuItem),
    Separator,
}

impl From<StandardItem> for MenuItem {
    fn from(item: StandardItem) -> Self {
        Self::Standard(item)
    }
}

impl From<CheckmarkItem> for MenuItem {
    fn from(item: CheckmarkItem) -> Self {
        Self::Checkmark(item)
    }
}

impl From<SubMenuItem> for MenuItem {
    fn from(item: SubMenuItem) -> Self {
        Self::SubMenu(item)
    }
}

impl From<muda::MenuItemKind> for MenuItem {
    fn from(item: muda::MenuItemKind) -> Self {
        match item {
            muda::MenuItemKind::MenuItem(menu_item) => StandardItem {
                id: menu_item.id().0.clone(),
                label: menu_item.text().replace('&', ""),
                enabled: menu_item.is_enabled(),
                icon: None,
                predefined_menu_item_type: None,
            }
            .into(),
            muda::MenuItemKind::Submenu(submenu) => SubMenuItem {
                label: submenu.text().replace('&', ""),
                enabled: submenu.is_enabled(),
                submenu: submenu.items().into_iter().map(Into::into).collect(),
            }
            .into(),
            muda::MenuItemKind::Predefined(predefined_menu_item) => {
                match predefined_menu_item.predefined_item_type() {
                    Some(PredefinedMenuItemType::Separator) => MenuItem::Separator,
                    Some(predefined_menu_item_type) => StandardItem {
                        id: predefined_menu_item.id().0.clone(),
                        label: predefined_menu_item.text().replace('&', ""),
                        enabled: true,
                        icon: None,
                        predefined_menu_item_type: Some(predefined_menu_item_type),
                    }
                    .into(),
                    _ => StandardItem {
                        id: predefined_menu_item.id().0.clone(),
                        label: predefined_menu_item.text().replace('&', ""),
                        enabled: true,
                        icon: None,
                        predefined_menu_item_type: None,
                    }
                    .into(),
                }
            }
            muda::MenuItemKind::Check(check_menu_item) => CheckmarkItem {
                id: check_menu_item.id().0.clone(),
                label: check_menu_item.text().replace('&', ""),
                enabled: check_menu_item.is_enabled(),
                checked: check_menu_item.is_checked(),
            }
            .into(),
            muda::MenuItemKind::Icon(icon_menu_item) => StandardItem {
                id: icon_menu_item.id().0.clone(),
                label: icon_menu_item.text().replace('&', ""),
                enabled: icon_menu_item.is_enabled(),
                icon: icon_menu_item.icon().map(|icon| icon.to_png()),
                predefined_menu_item_type: None,
            }
            .into(),
        }
    }
}

impl From<MenuItem> for ksni::MenuItem<Tray> {
    fn from(item: MenuItem) -> Self {
        match item {
            MenuItem::Standard(menu_item) => {
                let id = menu_item.id;
                match menu_item.predefined_menu_item_type {
                    Some(PredefinedMenuItemType::About(Some(metadata))) => {
                        let about_dialog = AboutDialog::new(metadata);
                        ksni::menu::StandardItem {
                            label: menu_item.label,
                            enabled: menu_item.enabled,
                            icon_data: menu_item.icon.unwrap_or_default(),
                            activate: Box::new(move |_| {
                                about_dialog.show();
                            }),
                            ..Default::default()
                        }
                        .into()
                    }
                    _ => ksni::menu::StandardItem {
                        label: menu_item.label,
                        enabled: menu_item.enabled,
                        icon_data: menu_item.icon.unwrap_or_default(),
                        activate: Box::new(move |_| send_menu_event(&id)),
                        ..Default::default()
                    }
                    .into(),
                }
            }
            MenuItem::Checkmark(check_menu_item) => {
                let id = check_menu_item.id;
                ksni::menu::CheckmarkItem {
                    label: check_menu_item.label,
                    enabled: check_menu_item.enabled,
                    checked: check_menu_item.checked,
                    activate: Box::new(move |_| send_menu_event(&id)),
                    ..Default::default()
                }
                .into()
            }
            MenuItem::SubMenu(submenu) => ksni::menu::SubMenu {
                label: submenu.label,
                enabled: submenu.enabled,
                submenu: submenu.submenu.into_iter().map(Into::into).collect(),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator => ksni::menu::MenuItem::Separator,
        }
    }
}

fn send_menu_event(id: &str) {
    muda::MenuEvent::send(muda::MenuEvent {
        id: muda::MenuId(id.to_string()),
    })
}
