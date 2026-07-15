#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuEntry {
    Snippets,
    FullTime,
    Date,
    CompactTime,
    Separator,
    Json,
    Password,
    Md5,
    Config,
    Sync,
    Quit,
}

const LINUX_MENU_ORDER: &[MenuEntry] = &[
    MenuEntry::Snippets,
    MenuEntry::Separator,
    MenuEntry::FullTime,
    MenuEntry::Date,
    MenuEntry::CompactTime,
    MenuEntry::Separator,
    MenuEntry::Password,
    MenuEntry::Md5,
    MenuEntry::Json,
    MenuEntry::Separator,
    MenuEntry::Config,
    MenuEntry::Sync,
    MenuEntry::Quit,
];

const WINDOWS_MENU_ORDER: &[MenuEntry] = &[
    MenuEntry::Snippets,
    MenuEntry::Separator,
    MenuEntry::FullTime,
    MenuEntry::Date,
    MenuEntry::CompactTime,
    MenuEntry::Separator,
    MenuEntry::Json,
    MenuEntry::Password,
    MenuEntry::Md5,
    MenuEntry::Separator,
    MenuEntry::Config,
    MenuEntry::Separator,
    MenuEntry::Quit,
];

pub fn linux_menu_order() -> &'static [MenuEntry] {
    LINUX_MENU_ORDER
}

pub fn windows_menu_order() -> &'static [MenuEntry] {
    WINDOWS_MENU_ORDER
}

#[cfg(test)]
mod tests {
    use super::{linux_menu_order, windows_menu_order, MenuEntry};

    #[test]
    fn linux_menu_order_starts_with_snippets() {
        assert_eq!(linux_menu_order().first(), Some(&MenuEntry::Snippets));
    }

    #[test]
    fn windows_menu_order_starts_with_snippets() {
        assert_eq!(windows_menu_order().first(), Some(&MenuEntry::Snippets));
    }
}
