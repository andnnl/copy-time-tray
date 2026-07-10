use gtk::prelude::*;
use gtk::{ComboBoxText, Dialog, DialogFlags, Entry, ResponseType};

use crate::snippet::config::{GiteeConfig, SyncProvider};

#[derive(Debug, Clone)]
pub struct ConfigDialogResult {
    pub provider: SyncProvider,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
    pub gitee_config: GiteeConfig,
}

#[derive(Clone, Debug)]
struct GithubFields {
    owner_entry: Entry,
    repo_entry: Entry,
    branch_entry: Entry,
    token_entry: Entry,
    box_widget: gtk::Box,
}

#[derive(Clone, Debug)]
struct GiteeFields {
    api_base_entry: Entry,
    token_entry: Entry,
    box_widget: gtk::Box,
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

    let owner_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let owner_label = gtk::Label::new(Some("Owner:"));
    owner_label.set_xalign(0.0);
    let owner_entry = Entry::new();
    owner_entry.set_placeholder_text(Some("GitHub 用户名"));
    owner_box.pack_start(&owner_label, false, false, 0);
    owner_box.pack_start(&owner_entry, true, true, 0);
    gh_box.pack_start(&owner_box, false, false, 0);

    let repo_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let repo_label = gtk::Label::new(Some("Repo:"));
    repo_label.set_xalign(0.0);
    let repo_entry = Entry::new();
    repo_entry.set_placeholder_text(Some("仓库名称"));
    repo_box.pack_start(&repo_label, false, false, 0);
    repo_box.pack_start(&repo_entry, true, true, 0);
    gh_box.pack_start(&repo_box, false, false, 0);

    let branch_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let branch_label = gtk::Label::new(Some("Branch:"));
    branch_label.set_xalign(0.0);
    let branch_entry = Entry::new();
    branch_entry.set_placeholder_text(Some("分支名称，如 main"));
    branch_entry.set_text("main");
    branch_box.pack_start(&branch_label, false, false, 0);
    branch_box.pack_start(&branch_entry, true, true, 0);
    gh_box.pack_start(&branch_box, false, false, 0);

    let gh_token_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let gh_token_label = gtk::Label::new(Some("Token:"));
    gh_token_label.set_xalign(0.0);
    let gh_token_entry = Entry::new();
    gh_token_entry.set_placeholder_text(Some("GitHub Personal Access Token"));
    gh_token_entry.set_visibility(false);
    gh_token_box.pack_start(&gh_token_label, false, false, 0);
    gh_token_box.pack_start(&gh_token_entry, true, true, 0);
    gh_box.pack_start(&gh_token_box, false, false, 0);

    content.pack_start(&gh_box, false, false, 0);

    let gitee_box = gtk::Box::new(gtk::Orientation::Vertical, 8);

    let api_base_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let api_base_label = gtk::Label::new(Some("API Base:"));
    api_base_label.set_xalign(0.0);
    let api_base_entry = Entry::new();
    api_base_entry.set_text("https://gitee.com/api/v5");
    api_base_entry.set_placeholder_text(Some("Gitee API 地址"));
    api_base_box.pack_start(&api_base_label, false, false, 0);
    api_base_box.pack_start(&api_base_entry, true, true, 0);
    gitee_box.pack_start(&api_base_box, false, false, 0);

    let gitee_token_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let gitee_token_label = gtk::Label::new(Some("Token:"));
    gitee_token_label.set_xalign(0.0);
    let gitee_token_entry = Entry::new();
    gitee_token_entry.set_placeholder_text(Some("Gitee 私人令牌"));
    gitee_token_entry.set_visibility(false);
    gitee_token_box.pack_start(&gitee_token_label, false, false, 0);
    gitee_token_box.pack_start(&gitee_token_entry, true, true, 0);
    gitee_box.pack_start(&gitee_token_box, false, false, 0);

    content.pack_start(&gitee_box, false, false, 0);

    gitee_box.set_no_show_all(true);
    gitee_box.hide();

    let gitee_box_toggle = gitee_box.clone();
    let gh_box_toggle = gh_box.clone();

    let gh_fields = GithubFields {
        owner_entry,
        repo_entry,
        branch_entry,
        token_entry: gh_token_entry,
        box_widget: gh_box,
    };

    let gitee_fields = GiteeFields {
        api_base_entry,
        token_entry: gitee_token_entry,
        box_widget: gitee_box,
    };
    provider_combo.connect_changed(move |combo| {
        let is_gitee = combo.active_text().as_deref() == Some("gitee");
        if is_gitee {
            gitee_box_toggle.set_no_show_all(false);
            gitee_box_toggle.show_all();
            gh_box_toggle.hide();
        } else {
            gh_box_toggle.set_no_show_all(false);
            gh_box_toggle.show_all();
            gitee_box_toggle.hide();
        }
    });

    let gitee_api_for_test = gitee_fields.api_base_entry.clone();
    let gitee_token_for_test = gitee_fields.token_entry.clone();
    let provider_for_test = provider_combo.clone();
    let parent_window: Option<gtk::Window> = parent.cloned();
    dialog.connect_response(move |_, resp| {
        if resp == ResponseType::Apply {
            let is_gitee = provider_for_test.active_text().as_deref() == Some("gitee");
            let pw = parent_window.as_ref().map(|w| w as &gtk::Window);
            if !is_gitee {
                show_error_dialog(pw, "测试连接仅支持 Gitee");
                return;
            }
            let api_base = gitee_api_for_test.text().to_string();
            let token = gitee_token_for_test.text().to_string();
            if api_base.is_empty() || token.is_empty() {
                show_error_dialog(pw, "请填写 API 地址和令牌");
                return;
            }
            let config = GiteeConfig {
                api_base,
                token,
                ..GiteeConfig::default()
            };
            std::thread::spawn(move || {
                let client = crate::snippet::gitee::GiteeClient::new(config);
                match client.list_gists() {
                    Ok(gists) => {
                        let _ = notify_rust::Notification::new()
                            .summary("连接成功")
                            .body(&format!("连接 Gitee 成功，共 {} 个片段", gists.len()))
                            .show();
                    }
                    Err(e) => {
                        let _ = notify_rust::Notification::new()
                            .summary("连接失败")
                            .body(&e.to_string())
                            .show();
                    }
                }
            });
        }
    });

    content.show_all();

    let result = dialog.run();
    dialog.hide();

    if result == ResponseType::Ok {
        let provider = match provider_combo.active_text() {
            Some(text) if text == "gitee" => SyncProvider::Gitee,
            _ => SyncProvider::Github,
        };

        match provider {
            SyncProvider::Gitee => {
                let api_base = gitee_fields.api_base_entry.text().to_string();
                let token = gitee_fields.token_entry.text().to_string();
                if api_base.is_empty() || token.is_empty() {
                    show_error_dialog(parent, "请填写 API 地址和令牌");
                    return None;
                }
                Some(ConfigDialogResult {
                    provider,
                    owner: String::new(),
                    repo: String::new(),
                    branch: String::new(),
                    token: String::new(),
                    gitee_config: GiteeConfig {
                        api_base,
                        token,
                        auto_sync: true,
                        sync_interval: 300,
                        gist_id: None,
                        gist_description: "CopyTimeTrayConfig".to_string(),
                    },
                })
            }
            SyncProvider::Github => {
                let owner = gh_fields.owner_entry.text().to_string();
                let repo = gh_fields.repo_entry.text().to_string();
                let branch = gh_fields.branch_entry.text().to_string();
                let token = gh_fields.token_entry.text().to_string();
                if owner.is_empty() || repo.is_empty() || branch.is_empty() || token.is_empty() {
                    show_error_dialog(parent, "请填写所有 GitHub 字段");
                    return None;
                }
                Some(ConfigDialogResult {
                    provider,
                    owner,
                    repo,
                    branch,
                    token,
                    gitee_config: GiteeConfig::default(),
                })
            }
        }
    } else {
        None
    }
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
