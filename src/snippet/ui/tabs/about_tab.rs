use gtk::prelude::*;
use super::ConfigTab;

pub struct AboutTab {
    #[allow(dead_code)]
    container: gtk::Box,
}

impl AboutTab {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 12);
        container.set_margin_top(20);
        container.set_margin_bottom(20);
        container.set_margin_start(20);
        container.set_margin_end(20);

        Self { container }
    }
}

impl ConfigTab for AboutTab {
    fn title(&self) -> &str {
        "关于"
    }

    fn build(&self, container: &gtk::Box) {
        // App name
        let name_label = gtk::Label::new(Some("Copy Time Tray"));
        name_label.style_context().add_class("title-1");
        name_label.set_halign(gtk::Align::Center);
        container.pack_start(&name_label, false, false, 0);

        // Version
        let version = env!("CARGO_PKG_VERSION");
        let version_label = gtk::Label::new(Some(&format!("版本: {}", version)));
        version_label.set_halign(gtk::Align::Center);
        container.pack_start(&version_label, false, false, 0);

        // Links
        let links_box = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        links_box.set_halign(gtk::Align::Center);
        links_box.set_margin_top(16);

        let github_link = gtk::LinkButton::with_label(
            "https://github.com/your-username/copy-time-tray",
            Some("GitHub")
        );
        links_box.pack_start(&github_link, false, false, 0);

        container.pack_start(&links_box, false, false, 0);

        // License
        let license_label = gtk::Label::new(Some("MIT License"));
        license_label.set_halign(gtk::Align::Center);
        license_label.set_margin_top(16);
        container.pack_start(&license_label, false, false, 0);

        // Description
        let desc_label = gtk::Label::new(Some(
            "一个简洁的系统托盘工具，用于快速复制时间、生成密码和管理代码片段。"
        ));
        desc_label.set_halign(gtk::Align::Center);
        desc_label.set_line_wrap(true);
        desc_label.set_max_width_chars(40);
        desc_label.set_margin_top(8);
        container.pack_start(&desc_label, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        // About tab has nothing to save
        Ok(())
    }

    fn on_load(&self) {
        // About tab has nothing to load
    }
}
