use gtk::prelude::*;
use gtk::{ComboBoxText, Dialog, DialogFlags, Entry, ResponseType};

use crate::snippet::config::{GiteeConfig, GithubConfig, SyncProvider};

#[derive(Debug, Clone)]
pub struct ConfigDialogResult {
    pub provider: SyncProvider,
    pub github_config: GithubConfig,
    pub gitee_config: GiteeConfig,
}

#[derive(Clone, Debug)]
struct GithubFields {
    token_entry: Entry,
    gist_description_entry: Entry,
    box_widget: gtk::Box,
}

#[derive(Clone, Debug)]
struct GiteeFields {
    api_base_entry: Entry,
    token_entry: Entry,
    box_widget: gtk::Box,
}

fn validate_github_fields(token: &str, gist_description: &str) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("请填写 GitHub Token".to_string());
    }
    if gist_description.trim().is_empty() {
        return Err("请填写 Gist 名称".to_string());
    }
    Ok(())
}

fn test_github_connection(api_base: &str, token: &str) -> Result<String, String> {
    let url = format!("{}/gists?per_page=1", api_base.trim_end_matches('/'));
    let response = reqwest::blocking::Client::new()
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "copy-time-tray/0.3")
        .send()
        .map_err(|error| format!("请求失败: {}", error))?;
    if response.status().is_success() {
        Ok("连接 GitHub Gist 成功".to_string())
    } else {
        Err(format!("认证失败，状态码: {}", response.status()))
    }
}

pub fn show_config_dialog(parent: Option<&gtk::Window>) -> Option<ConfigDialogResult> {
    let dialog = Dialog::with_buttons(
        Some("配置同步"),
        parent,
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("测试连接", ResponseType::Apply),
            ("保存", ResponseType::Ok),
            ("取消", ResponseType::Cancel),
        ],
    );
    dialog.set_default_size(400, -1);
    dialog.set_resizable(false);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let provider_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let provider_label = gtk::Label::new(Some("Provider:"));
    provider_label.set_xalign(0.0);
    let provider_combo = ComboBoxText::new();
    provider_combo.append_text("github");
    provider_combo.append_text("gitee");
    provider_combo.set_active(Some(0));
    provider_box.pack_start(&provider_label, false, false, 0);
    provider_box.pack_start(&provider_combo, true, true, 0);
    content.pack_start(&provider_box, false, false, 0);

    let gh_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let gh_token_entry = Entry::new();
    gh_token_entry.set_placeholder_text(Some("GitHub Personal Access Token"));
    gh_token_entry.set_visibility(false);
    gh_box.pack_start(&create_row("Token:", &gh_token_entry), false, false, 0);
    let gh_gist_description_entry = Entry::new();
    gh_gist_description_entry.set_text("CopyTimeTrayConfig");
    gh_gist_description_entry.set_placeholder_text(Some("Gist 描述名称"));
    gh_box.pack_start(
        &create_row("Gist 名称:", &gh_gist_description_entry),
        false,
        false,
        0,
    );
    let gh_help = gtk::Label::new(Some(
        "需要 classic PAT 的 gist scope，或 fine-grained PAT 的 account-level Gists: write 权限。",
    ));
    gh_help.set_line_wrap(true);
    gh_help.set_xalign(0.0);
    gh_help.style_context().add_class("dim-label");
    gh_box.pack_start(&gh_help, false, false, 0);
    content.pack_start(&gh_box, false, false, 0);

    let gitee_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let api_base_entry = Entry::new();
    api_base_entry.set_text("https://gitee.com/api/v5");
    api_base_entry.set_placeholder_text(Some("Gitee API 地址"));
    gitee_box.pack_start(&create_row("API Base:", &api_base_entry), false, false, 0);
    let gitee_token_entry = Entry::new();
    gitee_token_entry.set_placeholder_text(Some("Gitee 私人令牌"));
    gitee_token_entry.set_visibility(false);
    gitee_box.pack_start(&create_row("Token:", &gitee_token_entry), false, false, 0);
    content.pack_start(&gitee_box, false, false, 0);
    gitee_box.set_no_show_all(true);
    gitee_box.hide();

    let gh_fields = GithubFields {
        token_entry: gh_token_entry,
        gist_description_entry: gh_gist_description_entry,
        box_widget: gh_box,
    };
    let gitee_fields = GiteeFields {
        api_base_entry,
        token_entry: gitee_token_entry,
        box_widget: gitee_box,
    };

    let gh_box_toggle = gh_fields.box_widget.clone();
    let gitee_box_toggle = gitee_fields.box_widget.clone();
    provider_combo.connect_changed(move |combo| {
        let is_gitee = combo.active_text().as_deref() == Some("gitee");
        gh_box_toggle.set_no_show_all(is_gitee);
        gitee_box_toggle.set_no_show_all(!is_gitee);
        if is_gitee {
            gh_box_toggle.hide();
            gitee_box_toggle.show_all();
        } else {
            gitee_box_toggle.hide();
            gh_box_toggle.show_all();
        }
    });

    let provider_for_test = provider_combo.clone();
    let gh_token_for_test = gh_fields.token_entry.clone();
    let gh_gist_description_for_test = gh_fields.gist_description_entry.clone();
    let gitee_api_for_test = gitee_fields.api_base_entry.clone();
    let gitee_token_for_test = gitee_fields.token_entry.clone();
    let parent_window: Option<gtk::Window> = parent.cloned();
    dialog.connect_response(move |_, response| {
        if response != ResponseType::Apply {
            return;
        }
        let parent = parent_window.as_ref().map(|window| window as &gtk::Window);
        if provider_for_test.active_text().as_deref() == Some("gitee") {
            let api_base = gitee_api_for_test.text().to_string();
            let token = gitee_token_for_test.text().to_string();
            if api_base.is_empty() || token.is_empty() {
                show_error_dialog(parent, "请填写 API 地址和令牌");
                return;
            }
            std::thread::spawn(move || {
                let client = crate::snippet::gitee::GiteeClient::new(GiteeConfig {
                    api_base,
                    token,
                    ..GiteeConfig::default()
                });
                let body = match client.list_gists() {
                    Ok(gists) => format!("连接 Gitee 成功，共 {} 个片段", gists.len()),
                    Err(error) => error.to_string(),
                };
                let summary = if body.starts_with("连接 Gitee") {
                    "连接成功"
                } else {
                    "连接失败"
                };
                let _ = notify_rust::Notification::new()
                    .summary(summary)
                    .body(&body)
                    .show();
            });
        } else {
            let token = gh_token_for_test.text().to_string();
            let gist_description = gh_gist_description_for_test.text().to_string();
            if let Err(error) = validate_github_fields(&token, &gist_description) {
                show_error_dialog(parent, &error);
                return;
            }
            std::thread::spawn(move || {
                let result = test_github_connection("https://api.github.com", &token);
                let (summary, body) = match result {
                    Ok(message) => ("连接成功", message),
                    Err(error) => ("连接失败", error),
                };
                let _ = notify_rust::Notification::new()
                    .summary(summary)
                    .body(&body)
                    .show();
            });
        }
    });

    content.show_all();
    let result = dialog.run();
    dialog.hide();
    if result != ResponseType::Ok {
        return None;
    }

    match provider_combo.active_text().as_deref() {
        Some("gitee") => {
            let api_base = gitee_fields.api_base_entry.text().to_string();
            let token = gitee_fields.token_entry.text().to_string();
            if api_base.is_empty() || token.is_empty() {
                show_error_dialog(parent, "请填写 API 地址和令牌");
                return None;
            }
            Some(ConfigDialogResult {
                provider: SyncProvider::Gitee,
                github_config: GithubConfig::default(),
                gitee_config: GiteeConfig {
                    api_base,
                    token,
                    ..GiteeConfig::default()
                },
            })
        }
        _ => {
            let token = gh_fields.token_entry.text().to_string();
            let gist_description = gh_fields.gist_description_entry.text().to_string();
            if let Err(error) = validate_github_fields(&token, &gist_description) {
                show_error_dialog(parent, &error);
                return None;
            }
            Some(ConfigDialogResult {
                provider: SyncProvider::Github,
                github_config: GithubConfig {
                    token,
                    gist_description,
                    ..GithubConfig::default()
                },
                gitee_config: GiteeConfig::default(),
            })
        }
    }
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

pub fn show_error_dialog(parent: Option<&gtk::Window>, message: &str) {
    let dialog = Dialog::with_buttons(
        Some("错误"),
        parent,
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[("确定", ResponseType::Ok)],
    );
    dialog.set_default_size(300, -1);
    let content = dialog.content_area();
    let label = gtk::Label::new(Some(message));
    label.set_line_wrap(true);
    label.set_max_width_chars(50);
    content.pack_start(&label, true, true, 12);
    content.show_all();
    dialog.run();
    dialog.hide();
}

pub fn show_info_dialog(parent: Option<&gtk::Window>, title: &str, message: &str) {
    let dialog = Dialog::with_buttons(
        Some(title),
        parent,
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[("确定", ResponseType::Ok)],
    );
    dialog.set_default_size(300, -1);
    let content = dialog.content_area();
    let label = gtk::Label::new(Some(message));
    label.set_line_wrap(true);
    label.set_max_width_chars(50);
    content.pack_start(&label, true, true, 12);
    content.show_all();
    dialog.run();
    dialog.hide();
}
