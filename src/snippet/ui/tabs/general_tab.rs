use gtk::prelude::*;
use crate::snippet::config::Config;
use super::ConfigTab;

pub struct GeneralTab {
    pub auto_sync_switch: gtk::Switch,
    pub sync_interval_entry: gtk::Entry,
    pub notify_success_switch: gtk::Switch,
    pub notify_failure_switch: gtk::Switch,
    pub notify_create_switch: gtk::Switch,
}

impl GeneralTab {
    pub fn new() -> Self {
        let auto_sync_switch = gtk::Switch::new();
        auto_sync_switch.set_active(true);

        let sync_interval_entry = gtk::Entry::new();
        sync_interval_entry.set_text("300");
        sync_interval_entry.set_width_chars(8);

        let notify_success_switch = gtk::Switch::new();
        notify_success_switch.set_active(true);

        let notify_failure_switch = gtk::Switch::new();
        notify_failure_switch.set_active(true);

        let notify_create_switch = gtk::Switch::new();
        notify_create_switch.set_active(true);

        Self {
            auto_sync_switch,
            sync_interval_entry,
            notify_success_switch,
            notify_failure_switch,
            notify_create_switch,
        }
    }

    fn create_switch_row(label_text: &str, switch: &gtk::Switch) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(0.0);
        row.pack_start(&label, true, true, 0);
        row.pack_start(switch, false, false, 0);
        row
    }
}

impl ConfigTab for GeneralTab {
    fn title(&self) -> &str {
        "通用设置"
    }

    fn build(&self, container: &gtk::Box) {
        // Sync settings
        let sync_frame = gtk::Frame::new(Some("同步设置"));
        let sync_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sync_box.set_margin_top(8);
        sync_box.set_margin_bottom(8);
        sync_box.set_margin_start(8);
        sync_box.set_margin_end(8);

        sync_box.pack_start(
            &Self::create_switch_row("启动时自动同步", &self.auto_sync_switch),
            false, false, 0
        );

        let interval_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let interval_label = gtk::Label::new(Some("同步间隔:"));
        interval_label.set_xalign(0.0);
        interval_row.pack_start(&interval_label, true, true, 0);
        interval_row.pack_start(&self.sync_interval_entry, false, false, 0);
        let sec_label = gtk::Label::new(Some("秒"));
        interval_row.pack_start(&sec_label, false, false, 0);
        sync_box.pack_start(&interval_row, false, false, 0);

        sync_frame.add(&sync_box);
        container.pack_start(&sync_frame, false, false, 0);

        // Notification settings
        let notify_frame = gtk::Frame::new(Some("通知设置"));
        let notify_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        notify_box.set_margin_top(8);
        notify_box.set_margin_bottom(8);
        notify_box.set_margin_start(8);
        notify_box.set_margin_end(8);

        notify_box.pack_start(
            &Self::create_switch_row("同步成功时通知", &self.notify_success_switch),
            false, false, 0
        );
        notify_box.pack_start(
            &Self::create_switch_row("同步失败时通知", &self.notify_failure_switch),
            false, false, 0
        );
        notify_box.pack_start(
            &Self::create_switch_row("创建 Snippet 时通知", &self.notify_create_switch),
            false, false, 0
        );

        notify_frame.add(&notify_box);
        container.pack_start(&notify_frame, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        let interval_text = self.sync_interval_entry.text().to_string();
        let interval: u64 = interval_text.parse().map_err(|_| "同步间隔必须是数字".to_string())?;
        if interval < 10 {
            return Err("同步间隔不能小于 10 秒".to_string());
        }
        Ok(())
    }

    fn on_load(&self) {
        let config = Config::load().unwrap_or_default();
        let general = &config.general;

        self.auto_sync_switch.set_active(general.auto_sync);
        self.sync_interval_entry.set_text(&general.sync_interval.to_string());
        self.notify_success_switch.set_active(general.notify_on_success);
        self.notify_failure_switch.set_active(general.notify_on_failure);
        self.notify_create_switch.set_active(general.notify_on_create);
    }
}
