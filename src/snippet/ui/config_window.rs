use super::tabs::sync_tab::build_github_sync_config;
use super::tabs::{about_tab, general_tab, sync_tab, ConfigTab};
use crate::snippet::config::{Config, SyncConfig, SyncProvider};
use gtk::prelude::*;

pub struct ConfigWindow {
    window: gtk::Dialog,
    #[allow(dead_code)]
    notebook: gtk::Notebook,
    sync_tab: sync_tab::SyncTab,
    general_tab: general_tab::GeneralTab,
    #[allow(dead_code)]
    about_tab: about_tab::AboutTab,
}

impl ConfigWindow {
    pub fn new(parent: Option<&gtk::Window>) -> Self {
        let window = gtk::Dialog::with_buttons(
            Some("配置"),
            parent,
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            &[
                ("保存", gtk::ResponseType::Ok),
                ("取消", gtk::ResponseType::Cancel),
            ],
        );
        window.set_default_size(450, 500);
        window.set_resizable(true);
        window.set_position(gtk::WindowPosition::Center);

        let notebook = gtk::Notebook::new();

        let sync_tab = sync_tab::SyncTab::new();
        let general_tab = general_tab::GeneralTab::new();
        let about_tab = about_tab::AboutTab::new();

        // Build tabs
        let sync_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sync_container.set_margin_top(12);
        sync_container.set_margin_bottom(12);
        sync_container.set_margin_start(12);
        sync_container.set_margin_end(12);
        sync_tab.build(&sync_container);
        notebook.append_page(
            &sync_container,
            Some(&gtk::Label::new(Some(sync_tab.title()))),
        );

        let general_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        general_container.set_margin_top(12);
        general_container.set_margin_bottom(12);
        general_container.set_margin_start(12);
        general_container.set_margin_end(12);
        general_tab.build(&general_container);
        notebook.append_page(
            &general_container,
            Some(&gtk::Label::new(Some(general_tab.title()))),
        );

        let about_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        about_container.set_margin_top(12);
        about_container.set_margin_bottom(12);
        about_container.set_margin_start(12);
        about_container.set_margin_end(12);
        about_tab.build(&about_container);
        notebook.append_page(
            &about_container,
            Some(&gtk::Label::new(Some(about_tab.title()))),
        );

        let content = window.content_area();
        content.pack_start(&notebook, true, true, 0);

        // Load current config into tabs
        sync_tab.on_load();
        general_tab.on_load();
        about_tab.on_load();

        Self {
            window,
            notebook,
            sync_tab,
            general_tab,
            about_tab,
        }
    }

    fn build_sync_config_from_dialog(dialog: &ConfigWindow) -> Option<SyncConfig> {
        let existing_sync_config = Config::load().ok().and_then(|cfg| cfg.sync);
        let provider = match dialog.sync_tab.provider_combo.active_text().as_deref() {
            Some("gitee") => SyncProvider::Gitee,
            _ => SyncProvider::Github,
        };

        Some(match provider {
            SyncProvider::Github => {
                let existing_github_config = existing_sync_config
                    .as_ref()
                    .map(|sync| sync.github_config.clone())
                    .unwrap_or_default();
                let existing_gitee_config = existing_sync_config
                    .as_ref()
                    .map(|sync| sync.gitee_config.clone())
                    .unwrap_or_default();
                build_github_sync_config(
                    dialog.sync_tab.gh_token_entry.text().as_str(),
                    dialog.sync_tab.gh_gist_desc_entry.text().as_str(),
                    &existing_github_config,
                    &existing_gitee_config,
                )
            }
            SyncProvider::Gitee => {
                let existing_github_config = existing_sync_config
                    .as_ref()
                    .map(|sync| sync.github_config.clone())
                    .unwrap_or_default();
                let existing_gitee_config = existing_sync_config
                    .as_ref()
                    .map(|sync| sync.gitee_config.clone())
                    .unwrap_or_default();
                sync_tab::build_gitee_sync_config(
                    dialog.sync_tab.gitee_api_entry.text().as_str(),
                    dialog.sync_tab.gitee_token_entry.text().as_str(),
                    dialog.sync_tab.gitee_gist_desc_entry.text().as_str(),
                    &existing_github_config,
                    &existing_gitee_config,
                )
            }
        })
    }

    pub fn show(parent: Option<&gtk::Window>) -> Option<Config> {
        let dialog = Self::new(parent);
        dialog.window.show_all();

        let result = dialog.window.run();
        dialog.window.hide();

        if result == gtk::ResponseType::Ok {
            // Validate all tabs
            if let Err(e) = dialog.sync_tab.on_save() {
                show_error_dialog(parent, &e);
                return None;
            }
            if let Err(e) = dialog.general_tab.on_save() {
                show_error_dialog(parent, &e);
                return None;
            }

            let sync_config = Self::build_sync_config_from_dialog(&dialog);

            let general_config = crate::snippet::config::GeneralConfig {
                auto_sync: dialog.general_tab.auto_sync_switch.is_active(),
                sync_interval: dialog
                    .general_tab
                    .sync_interval_entry
                    .text()
                    .to_string()
                    .parse()
                    .unwrap_or(300),
                notify_on_success: dialog.general_tab.notify_success_switch.is_active(),
                notify_on_failure: dialog.general_tab.notify_failure_switch.is_active(),
                notify_on_create: dialog.general_tab.notify_create_switch.is_active(),
            };

            let mut config = Config::load().unwrap_or_default();
            config.sync = sync_config;
            config.general = general_config;

            Some(config)
        } else {
            None
        }
    }
}

fn show_error_dialog(parent: Option<&gtk::Window>, message: &str) {
    let dialog = gtk::Dialog::with_buttons(
        Some("错误"),
        parent,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("确定", gtk::ResponseType::Ok)],
    );
    dialog.set_default_size(300, -1);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();
    let label = gtk::Label::new(Some(message));
    label.set_line_wrap(true);
    label.set_max_width_chars(50);
    content.pack_start(&label, true, true, 12);
    content.show_all();

    dialog.run();
    dialog.hide();
}
