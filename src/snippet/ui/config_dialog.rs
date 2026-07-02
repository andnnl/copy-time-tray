use gtk::prelude::*;
use gtk::{Dialog, DialogFlags, Entry, ComboBoxText, ResponseType};

use crate::snippet::config::{SyncProvider};

#[derive(Debug, Clone)]
pub struct ConfigDialogResult {
    pub provider: SyncProvider,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
}

pub fn show_config_dialog(parent: Option<&gtk::Window>) -> Option<ConfigDialogResult> {
    let dialog = Dialog::with_buttons(
        Some("配置同步"),
        parent,
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[
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

    let owner_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let owner_label = gtk::Label::new(Some("Owner:"));
    owner_label.set_xalign(0.0);
    let owner_entry = Entry::new();
    owner_entry.set_placeholder_text(Some("GitHub/Gitee 用户名"));
    owner_box.pack_start(&owner_label, false, false, 0);
    owner_box.pack_start(&owner_entry, true, true, 0);
    content.pack_start(&owner_box, false, false, 0);

    let repo_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let repo_label = gtk::Label::new(Some("Repo:"));
    repo_label.set_xalign(0.0);
    let repo_entry = Entry::new();
    repo_entry.set_placeholder_text(Some("仓库名称"));
    repo_box.pack_start(&repo_label, false, false, 0);
    repo_box.pack_start(&repo_entry, true, true, 0);
    content.pack_start(&repo_box, false, false, 0);

    let branch_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let branch_label = gtk::Label::new(Some("Branch:"));
    branch_label.set_xalign(0.0);
    let branch_entry = Entry::new();
    branch_entry.set_placeholder_text(Some("分支名称，如 main"));
    branch_entry.set_text("main");
    branch_box.pack_start(&branch_label, false, false, 0);
    branch_box.pack_start(&branch_entry, true, true, 0);
    content.pack_start(&branch_box, false, false, 0);

    let token_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let token_label = gtk::Label::new(Some("Token:"));
    token_label.set_xalign(0.0);
    let token_entry = Entry::new();
    token_entry.set_placeholder_text(Some("Personal Access Token"));
    token_entry.set_visibility(false);
    token_box.pack_start(&token_label, false, false, 0);
    token_box.pack_start(&token_entry, true, true, 0);
    content.pack_start(&token_box, false, false, 0);

    content.show_all();

    let result = dialog.run();
    dialog.hide();

    if result == ResponseType::Ok {
        let provider = match provider_combo.active_text() {
            Some(text) if text == "gitee" => SyncProvider::Gitee,
            _ => SyncProvider::Github,
        };
        let owner = owner_entry.text().to_string();
        let repo = repo_entry.text().to_string();
        let branch = branch_entry.text().to_string();
        let token = token_entry.text().to_string();

        if owner.is_empty() || repo.is_empty() || branch.is_empty() || token.is_empty() {
            show_error_dialog(parent, "请填写所有字段");
            None
        } else {
            Some(ConfigDialogResult {
                provider,
                owner,
                repo,
                branch,
                token,
            })
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
