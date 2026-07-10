use super::ConfigTab;
use crate::snippet::config::{Config, SyncHistoryEntry, SyncProvider, SyncResultType, SyncStatus};
use gtk::glib;
use gtk::prelude::*;

fn show_test_result_dialog(parent: Option<&gtk::Window>, message: &str, is_success: bool) {
    let title = if is_success {
        "连接成功"
    } else {
        "连接失败"
    };
    let dialog = gtk::Dialog::with_buttons(
        Some(title),
        parent,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("确定", gtk::ResponseType::Ok)],
    );
    dialog.set_default_size(350, -1);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let icon = if is_success { "✓" } else { "✗" };
    let icon_label = gtk::Label::new(Some(icon));
    icon_label
        .style_context()
        .add_class(if is_success { "success" } else { "error" });
    icon_label.set_halign(gtk::Align::Center);
    content.pack_start(&icon_label, false, false, 0);

    let message_label = gtk::Label::new(Some(message));
    message_label.set_line_wrap(true);
    message_label.set_max_width_chars(50);
    message_label.set_halign(gtk::Align::Center);
    content.pack_start(&message_label, false, false, 0);

    content.show_all();
    dialog.run();
    dialog.hide();
}

fn test_github_connection(token: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "copy-time-tray")
        .send()
        .map_err(|e| format!("请求失败: {}", e))?;

    if response.status().is_success() {
        let user: serde_json::Value = response
            .json()
            .map_err(|e| format!("解析响应失败: {}", e))?;
        let login = user["login"].as_str().unwrap_or("未知用户");
        Ok(format!("连接成功！用户: {}", login))
    } else {
        let status = response.status();
        Err(format!("认证失败，状态码: {}", status))
    }
}

fn test_gitee_connection(api_base: &str, token: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/user?access_token={}", api_base, token);
    let response = client
        .get(&url)
        .header("User-Agent", "copy-time-tray")
        .send()
        .map_err(|e| format!("请求失败: {}", e))?;

    if response.status().is_success() {
        let user: serde_json::Value = response
            .json()
            .map_err(|e| format!("解析响应失败: {}", e))?;
        let login = user["login"].as_str().unwrap_or("未知用户");
        Ok(format!("连接成功！用户: {}", login))
    } else {
        let status = response.status();
        Err(format!("认证失败，状态码: {}", status))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManualSyncAction {
    ManagerPullThenPush,
}

pub struct SyncTab {
    pub provider_combo: gtk::ComboBoxText,
    pub github_box: gtk::Box,
    pub gitee_box: gtk::Box,
    pub owner_entry: gtk::Entry,
    pub repo_entry: gtk::Entry,
    pub branch_entry: gtk::Entry,
    pub gh_token_entry: gtk::Entry,
    pub gitee_api_entry: gtk::Entry,
    pub gitee_token_entry: gtk::Entry,
    pub gitee_gist_desc_entry: gtk::Entry,
    status_label: gtk::Label,
    history_box: gtk::Box,
    history_expander: gtk::Expander,
}

impl SyncTab {
    pub fn new() -> Self {
        let provider_combo = gtk::ComboBoxText::new();
        provider_combo.append_text("github");
        provider_combo.append_text("gitee");
        provider_combo.set_active(Some(0));

        // GitHub fields
        let gh_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let owner_entry = Self::create_entry("GitHub 用户名");
        let repo_entry = Self::create_entry("仓库名称");
        let branch_entry = Self::create_entry("分支名称");
        let gh_token_entry = Self::create_password_entry("GitHub Personal Access Token");

        gh_box.pack_start(&Self::create_row("Owner:", &owner_entry), false, false, 0);
        gh_box.pack_start(&Self::create_row("Repo:", &repo_entry), false, false, 0);
        gh_box.pack_start(&Self::create_row("Branch:", &branch_entry), false, false, 0);
        gh_box.pack_start(
            &Self::create_row("Token:", &gh_token_entry),
            false,
            false,
            0,
        );

        // GitHub test connection button
        let gh_test_button = gtk::Button::with_label("测试连接");
        let gh_token_for_test = gh_token_entry.clone();
        let gh_test_button_for_signal = gh_test_button.clone();
        gh_test_button.connect_clicked(move |button| {
            let token = gh_token_for_test.text().to_string();
            if token.is_empty() {
                show_test_result_dialog(
                    button
                        .toplevel()
                        .and_then(|w| w.downcast::<gtk::Window>().ok())
                        .as_ref(),
                    "请先填写 Token",
                    false,
                );
                return;
            }
            button.set_sensitive(false);
            button.set_label("测试中...");
            let (sender, receiver) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = test_github_connection(&token);
                let _ = sender.send_blocking(result);
            });
            let button_weak = button.downgrade();
            glib::spawn_future_local(async move {
                while let Ok(result) = receiver.recv().await {
                    if let Some(button) = button_weak.upgrade() {
                        button.set_sensitive(true);
                        button.set_label("测试连接");
                        let window = button
                            .toplevel()
                            .and_then(|w| w.downcast::<gtk::Window>().ok());
                        match result {
                            Ok(msg) => {
                                show_test_result_dialog(window.as_ref(), &msg, true);
                            }
                            Err(e) => {
                                show_test_result_dialog(window.as_ref(), &e, false);
                            }
                        }
                    }
                }
            });
        });
        gh_box.pack_start(&gh_test_button, false, false, 0);

        // Gitee fields
        let gitee_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let gitee_api_entry = Self::create_entry("Gitee API 地址");
        gitee_api_entry.set_text("https://gitee.com/api/v5");
        let gitee_token_entry = Self::create_password_entry("Gitee 私人令牌");
        let gitee_gist_desc_entry = Self::create_entry("Gist 描述名称");
        gitee_gist_desc_entry.set_text("CopyTimeTrayConfig");

        gitee_box.pack_start(
            &Self::create_row("API Base:", &gitee_api_entry),
            false,
            false,
            0,
        );
        gitee_box.pack_start(
            &Self::create_row("Token:", &gitee_token_entry),
            false,
            false,
            0,
        );
        gitee_box.pack_start(
            &Self::create_row("Gist 名称:", &gitee_gist_desc_entry),
            false,
            false,
            0,
        );

        // Gitee test connection button
        let gitee_test_button = gtk::Button::with_label("测试连接");
        let gitee_api_for_test = gitee_api_entry.clone();
        let gitee_token_for_test = gitee_token_entry.clone();
        let gitee_test_button_for_signal = gitee_test_button.clone();
        gitee_test_button.connect_clicked(move |button| {
            let api_base = gitee_api_for_test.text().to_string();
            let token = gitee_token_for_test.text().to_string();
            if api_base.is_empty() || token.is_empty() {
                show_test_result_dialog(
                    button
                        .toplevel()
                        .and_then(|w| w.downcast::<gtk::Window>().ok())
                        .as_ref(),
                    "请先填写 API 地址和 Token",
                    false,
                );
                return;
            }
            button.set_sensitive(false);
            button.set_label("测试中...");
            let (sender, receiver) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = test_gitee_connection(&api_base, &token);
                let _ = sender.send_blocking(result);
            });
            let button_weak = button.downgrade();
            glib::spawn_future_local(async move {
                while let Ok(result) = receiver.recv().await {
                    if let Some(button) = button_weak.upgrade() {
                        button.set_sensitive(true);
                        button.set_label("测试连接");
                        let window = button
                            .toplevel()
                            .and_then(|w| w.downcast::<gtk::Window>().ok());
                        match result {
                            Ok(msg) => {
                                show_test_result_dialog(window.as_ref(), &msg, true);
                            }
                            Err(e) => {
                                show_test_result_dialog(window.as_ref(), &e, false);
                            }
                        }
                    }
                }
            });
        });
        gitee_box.pack_start(&gitee_test_button, false, false, 0);

        // Status area
        let status_label = gtk::Label::new(Some("未配置"));
        status_label.set_xalign(0.0);

        // History area
        let history_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let history_expander = gtk::Expander::new(Some("同步历史"));
        history_expander.add(&history_box);

        // Toggle visibility based on provider. Hidden provider sections must also
        // use no_show_all=true, otherwise ConfigWindow::show_all() will reveal
        // them again before the user changes the provider.
        Self::apply_provider_visibility(
            &gh_box,
            &gitee_box,
            provider_combo.active_text().as_deref(),
        );

        let gitee_clone = gitee_box.clone();
        let gh_clone = gh_box.clone();
        provider_combo.connect_changed(move |combo| {
            SyncTab::apply_provider_visibility(
                &gh_clone,
                &gitee_clone,
                combo.active_text().as_deref(),
            );
        });

        Self {
            provider_combo,
            github_box: gh_box,
            gitee_box,
            owner_entry,
            repo_entry,
            branch_entry,
            gh_token_entry,
            gitee_api_entry,
            gitee_token_entry,
            gitee_gist_desc_entry,
            status_label,
            history_box,
            history_expander,
        }
    }

    fn provider_visibility(provider: Option<&str>) -> (bool, bool) {
        let show_gitee = provider == Some("gitee");
        (!show_gitee, show_gitee)
    }

    fn apply_provider_visibility(
        github_box: &gtk::Box,
        gitee_box: &gtk::Box,
        provider: Option<&str>,
    ) {
        let (show_github, show_gitee) = Self::provider_visibility(provider);
        if show_github {
            github_box.set_no_show_all(false);
            github_box.show_all();
        } else {
            github_box.set_no_show_all(true);
            github_box.hide();
        }

        if show_gitee {
            gitee_box.set_no_show_all(false);
            gitee_box.show_all();
        } else {
            gitee_box.set_no_show_all(true);
            gitee_box.hide();
        }
    }

    fn manual_sync_action(_provider: Option<&str>) -> ManualSyncAction {
        // The config dialog must use the same authoritative sync flow as the
        // tray menu: pull remote changes into the local tree first, then push
        // the merged full tree. Keeping a separate per-provider implementation
        // here previously skipped local merge/gist-id persistence and made the
        // button appear to do nothing.
        ManualSyncAction::ManagerPullThenPush
    }

    fn create_entry(placeholder: &str) -> gtk::Entry {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(placeholder));
        entry
    }

    fn create_password_entry(placeholder: &str) -> gtk::Entry {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(placeholder));
        entry.set_visibility(false);
        entry
    }

    fn create_row(label_text: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(0.0);
        label.set_width_chars(10);
        row.pack_start(&label, false, false, 0);
        row.pack_start(widget, true, true, 0);
        row
    }

    fn update_status_display(&self, status: &SyncStatus) {
        if let Some(time) = &status.last_sync_time {
            let result_text = match &status.last_sync_result {
                Some(SyncResultType::Success) => "✓ 成功",
                Some(SyncResultType::Failed) => "✗ 失败",
                None => "未知",
            };
            self.status_label.set_text(&format!(
                "上次同步: {}\n待同步: {} 个片段\n状态: {}",
                time.format("%Y-%m-%d %H:%M:%S"),
                status.pending_count,
                result_text
            ));
        } else {
            self.status_label.set_text("尚未同步");
        }
    }

    fn update_history_display(&self, history: &[SyncHistoryEntry]) {
        // Clear existing children
        for child in self.history_box.children() {
            self.history_box.remove(&child);
        }

        if history.is_empty() {
            let label = gtk::Label::new(Some("暂无同步记录"));
            label.set_xalign(0.0);
            self.history_box.pack_start(&label, false, false, 4);
        } else {
            for entry in history.iter().rev().take(10) {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
                let result_icon = match entry.result {
                    SyncResultType::Success => "✓",
                    SyncResultType::Failed => "✗",
                };
                let text = format!(
                    "{} {} {} {}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    entry.direction,
                    result_icon,
                    entry.result
                );
                let label = gtk::Label::new(Some(&text));
                label.set_xalign(0.0);
                row.pack_start(&label, false, false, 2);

                if let Some(error) = &entry.error {
                    let error_label = gtk::Label::new(Some(&format!("  错误: {}", error)));
                    error_label.set_xalign(0.0);
                    error_label.style_context().add_class("dim-label");
                    row.pack_start(&error_label, false, false, 2);
                }

                self.history_box.pack_start(&row, false, false, 4);
            }
        }
        self.history_box.show_all();
    }
}

impl ConfigTab for SyncTab {
    fn title(&self) -> &str {
        "同步"
    }

    fn build(&self, container: &gtk::Box) {
        // Provider selection
        let provider_row = Self::create_row("Provider:", &self.provider_combo);
        container.pack_start(&provider_row, false, false, 0);

        // GitHub config
        container.pack_start(&self.github_box, false, false, 0);

        // Gitee config
        container.pack_start(&self.gitee_box, false, false, 0);

        // Separator
        container.pack_start(
            &gtk::Separator::new(gtk::Orientation::Horizontal),
            false,
            false,
            8,
        );

        // Manual sync button
        let sync_button = gtk::Button::with_label("立即同步");
        sync_button.style_context().add_class("suggested-action");
        let sync_provider_combo = self.provider_combo.clone();
        let sync_gh_token = self.gh_token_entry.clone();
        let sync_gh_owner = self.owner_entry.clone();
        let sync_gh_repo = self.repo_entry.clone();
        let sync_gh_branch = self.branch_entry.clone();
        let sync_gitee_api = self.gitee_api_entry.clone();
        let sync_gitee_token = self.gitee_token_entry.clone();
        let sync_gitee_desc = self.gitee_gist_desc_entry.clone();
        let sync_status_label = self.status_label.clone();
        sync_button.connect_clicked(move |button| {
            let provider_binding = sync_provider_combo.active_text();
            let provider = provider_binding.as_deref().unwrap_or("github");
            let _action = SyncTab::manual_sync_action(Some(provider));

            let sync_config = if provider == "gitee" {
                let api_base = sync_gitee_api.text().to_string();
                let token = sync_gitee_token.text().to_string();
                let gist_desc = sync_gitee_desc.text().to_string();
                if api_base.is_empty() || token.is_empty() {
                    let _ = notify_rust::Notification::new()
                        .summary("同步失败")
                        .body("请先填写 Gitee API 地址和 Token")
                        .show();
                    return;
                }
                let existing_gist_id = Config::load()
                    .ok()
                    .and_then(|cfg| cfg.sync)
                    .and_then(|sync| sync.gitee_config.gist_id);
                crate::snippet::config::SyncConfig {
                    provider: crate::snippet::config::SyncProvider::Gitee,
                    owner: String::new(),
                    repo: String::new(),
                    branch: String::new(),
                    token: String::new(),
                    gitee_config: crate::snippet::config::GiteeConfig {
                        api_base,
                        token,
                        auto_sync: true,
                        sync_interval: 300,
                        gist_id: existing_gist_id,
                        gist_description: gist_desc,
                    },
                }
            } else {
                let owner = sync_gh_owner.text().to_string();
                let repo = sync_gh_repo.text().to_string();
                let branch = sync_gh_branch.text().to_string();
                let token = sync_gh_token.text().to_string();
                if owner.is_empty() || repo.is_empty() || branch.is_empty() || token.is_empty() {
                    let _ = notify_rust::Notification::new()
                        .summary("同步失败")
                        .body("请先填写所有 GitHub 字段")
                        .show();
                    return;
                }
                crate::snippet::config::SyncConfig {
                    provider: crate::snippet::config::SyncProvider::Github,
                    owner,
                    repo,
                    branch,
                    token,
                    gitee_config: crate::snippet::config::GiteeConfig::default(),
                }
            };

            button.set_sensitive(false);
            button.set_label("同步中...");
            let (sender, receiver) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = (|| -> Result<(), String> {
                    let mut config = Config::load().unwrap_or_default();
                    config.sync = Some(sync_config);
                    config.save().map_err(|e| e.to_string())?;

                    let manager_mutex = crate::app::MANAGER
                        .get()
                        .ok_or_else(|| "Manager not initialized".to_string())?;
                    let mut manager = manager_mutex.lock().map_err(|e| e.to_string())?;
                    manager.reload_config().map_err(|e| e.to_string())?;
                    manager.do_pull_with_retry()?;
                    manager.do_push()?;
                    Ok(())
                })();
                let _ = sender.send_blocking(result);
            });

            let button_weak = button.downgrade();
            let status_weak = sync_status_label.downgrade();
            glib::spawn_future_local(async move {
                while let Ok(result) = receiver.recv().await {
                    if let Some(button) = button_weak.upgrade() {
                        button.set_sensitive(true);
                        button.set_label("立即同步");
                    }
                    match result {
                        Ok(()) => {
                            let _ = notify_rust::Notification::new()
                                .summary("同步成功")
                                .body("已完成拉取、合并并上传")
                                .show();
                            if let Some(label) = status_weak.upgrade() {
                                label.set_text("同步完成");
                            }
                        }
                        Err(e) => {
                            let _ = notify_rust::Notification::new()
                                .summary("同步失败")
                                .body(&e)
                                .show();
                            if let Some(label) = status_weak.upgrade() {
                                label.set_text(&format!("同步失败：{}", e));
                            }
                        }
                    }
                }
            });
        });
        container.pack_start(&sync_button, false, false, 0);

        // Status area
        let status_frame = gtk::Frame::new(Some("同步状态"));
        status_frame.add(&self.status_label);
        container.pack_start(&status_frame, false, false, 0);

        // History
        container.pack_start(&self.history_expander, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        // Validation is done here
        let provider = match self.provider_combo.active_text() {
            Some(text) if text == "gitee" => SyncProvider::Gitee,
            _ => SyncProvider::Github,
        };

        match provider {
            SyncProvider::Github => {
                let owner = self.owner_entry.text().to_string();
                let repo = self.repo_entry.text().to_string();
                let branch = self.branch_entry.text().to_string();
                let token = self.gh_token_entry.text().to_string();
                if owner.is_empty() || repo.is_empty() || branch.is_empty() || token.is_empty() {
                    return Err("请填写所有 GitHub 字段".to_string());
                }
            }
            SyncProvider::Gitee => {
                let api_base = self.gitee_api_entry.text().to_string();
                let token = self.gitee_token_entry.text().to_string();
                let gist_desc = self.gitee_gist_desc_entry.text().to_string();
                if api_base.is_empty() || token.is_empty() {
                    return Err("请填写 API 地址和令牌".to_string());
                }
                if gist_desc.is_empty() {
                    return Err("请填写 Gist 名称".to_string());
                }
            }
        }

        Ok(())
    }

    fn on_load(&self) {
        let config = Config::load().unwrap_or_default();

        if let Some(sync_config) = &config.sync {
            match sync_config.provider {
                SyncProvider::Github => {
                    self.provider_combo.set_active(Some(0));
                    self.owner_entry.set_text(&sync_config.owner);
                    self.repo_entry.set_text(&sync_config.repo);
                    self.branch_entry.set_text(&sync_config.branch);
                    self.gh_token_entry.set_text(&sync_config.token);
                }
                SyncProvider::Gitee => {
                    self.provider_combo.set_active(Some(1));
                    self.gitee_api_entry
                        .set_text(&sync_config.gitee_config.api_base);
                    self.gitee_token_entry
                        .set_text(&sync_config.gitee_config.token);
                    self.gitee_gist_desc_entry
                        .set_text(&sync_config.gitee_config.gist_description);
                }
            }
        }

        Self::apply_provider_visibility(
            &self.github_box,
            &self.gitee_box,
            self.provider_combo.active_text().as_deref(),
        );

        // Load sync status
        let status = Config::load_sync_status().unwrap_or_default();
        self.update_status_display(&status);
        self.update_history_display(&status.history);
    }
}

#[cfg(test)]
mod tests {
    use super::{ManualSyncAction, SyncTab};

    #[test]
    fn test_provider_visibility_defaults_to_github_only() {
        assert_eq!(SyncTab::provider_visibility(None), (true, false));
        assert_eq!(SyncTab::provider_visibility(Some("github")), (true, false));
    }

    #[test]
    fn test_provider_visibility_gitee_only() {
        assert_eq!(SyncTab::provider_visibility(Some("gitee")), (false, true));
    }

    #[test]
    fn test_manual_sync_button_reuses_manager_flow_for_gitee() {
        assert_eq!(
            SyncTab::manual_sync_action(Some("gitee")),
            ManualSyncAction::ManagerPullThenPush
        );
    }

    #[test]
    fn test_manual_sync_button_reuses_manager_flow_for_github() {
        assert_eq!(
            SyncTab::manual_sync_action(Some("github")),
            ManualSyncAction::ManagerPullThenPush
        );
    }
}
