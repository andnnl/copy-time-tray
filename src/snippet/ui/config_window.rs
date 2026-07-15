use super::tabs::{about_tab, general_tab, sync_tab, ConfigTab};
use crate::snippet::config::{Config, GiteeConfig, SyncConfig, SyncProvider};
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
        let existing_gitee_config = Config::load()
            .ok()
            .and_then(|cfg| cfg.sync)
            .map(|sync| sync.gitee_config)
            .unwrap_or_default();

        Some(SyncConfig {
            provider: match dialog.sync_tab.provider_combo.active_text().as_deref() {
                Some("gitee") => SyncProvider::Gitee,
                _ => SyncProvider::Github,
            },
            owner: dialog.sync_tab.owner_entry.text().to_string(),
            repo: dialog.sync_tab.repo_entry.text().to_string(),
            branch: dialog.sync_tab.branch_entry.text().to_string(),
            token: dialog.sync_tab.gh_token_entry.text().to_string(),
            gitee_config: GiteeConfig {
                api_base: dialog.sync_tab.gitee_api_entry.text().to_string(),
                token: dialog.sync_tab.gitee_token_entry.text().to_string(),
                auto_sync: existing_gitee_config.auto_sync,
                sync_interval: existing_gitee_config.sync_interval,
                gist_id: existing_gitee_config.gist_id,
                gist_description: dialog.sync_tab.gitee_gist_desc_entry.text().to_string(),
            },
        })
    }

    fn build_config(dialog: &ConfigWindow) -> Config {
        let sync_config = Self::build_sync_config_from_dialog(dialog);
        let mut config = Config::load().unwrap_or_default();
        config.sync = sync_config;
        config.general = crate::snippet::config::GeneralConfig {
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
            snippet_hotkey: dialog.general_tab.snippet_hotkey(),
        };
        config
    }

    pub fn show<F>(parent: Option<&gtk::Window>, mut apply: F) -> bool
    where
        F: FnMut(Config) -> Result<(), String>,
    {
        let dialog = Self::new(parent);
        dialog.window.show_all();

        loop {
            if dialog.window.run() != gtk::ResponseType::Ok {
                dialog.window.hide();
                return false;
            }
            if let Err(error) = dialog.sync_tab.on_save() {
                show_error_dialog(parent, &error);
                continue;
            }
            if let Err(error) = dialog.general_tab.on_save() {
                dialog.general_tab.show_error(&error);
                continue;
            }
            match apply(Self::build_config(&dialog)) {
                Ok(()) => {
                    dialog
                        .general_tab
                        .show_success("快捷键绑定成功，配置已保存");
                    dialog.window.hide();
                    return true;
                }
                Err(error) => dialog.general_tab.show_error(&error),
            }
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
