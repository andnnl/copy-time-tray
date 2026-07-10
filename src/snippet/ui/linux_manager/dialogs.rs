use gtk::prelude::*;

#[cfg(target_os = "linux")]
pub(super) fn show_create_snippet_dialog(
    folders: &[(String, String)],
    default_parent_id: Option<&str>,
) -> Option<(String, String, Option<String>)> {
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("新建 Snippet"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("创建", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(400, 350);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);

    let folder_combo = gtk::ComboBoxText::new();
    folder_combo.append_text("(根目录)");
    for (_fid, fname) in folders {
        folder_combo.append_text(fname);
    }

    let active_idx = match default_parent_id {
        Some(pid) => {
            if let Some(pos) = folders.iter().position(|(id, _)| id == pid) {
                (pos + 1) as u32
            } else {
                0
            }
        }
        None => 0,
    };
    folder_combo.set_active(Some(active_idx));
    let folder_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    folder_row.pack_start(&gtk::Label::new(Some("所属目录：")), false, true, 0);
    folder_row.pack_start(&folder_combo, true, true, 0);
    content.pack_start(&folder_row, false, true, 0);

    let name_label = gtk::Label::new(Some("名称："));
    name_label.set_xalign(0.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("片段名称"));
    content.pack_start(&name_label, false, true, 0);
    content.pack_start(&name_entry, false, true, 0);

    let content_label = gtk::Label::new(Some("内容："));
    content_label.set_xalign(0.0);
    content_label.set_margin_top(8);
    let content_view = gtk::TextView::new();
    content_view.set_wrap_mode(gtk::WrapMode::Word);
    let scrolled = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrolled.set_min_content_height(200);
    scrolled.add(&content_view);
    content.pack_start(&content_label, false, true, 0);
    content.pack_start(&scrolled, true, true, 0);

    content.show_all();

    let response = dialog.run();

    let result = if response == gtk::ResponseType::Ok {
        let name = name_entry.text().to_string();
        let buffer = content_view.buffer().unwrap();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .unwrap_or_default()
            .to_string();
        let parent_id = match folder_combo.active() {
            Some(idx) if idx > 0 => folders.get(idx as usize - 1).map(|(id, _)| id.clone()),
            _ => None,
        };

        if name.is_empty() {
            None
        } else {
            Some((name, text, parent_id))
        }
    } else {
        None
    };

    dialog.close();
    result
}

#[cfg(target_os = "linux")]
pub(super) fn show_edit_snippet_dialog(
    existing_name: &str,
    existing_content: &str,
    folders: &[(String, String)],
    current_parent_id: Option<&str>,
) -> Option<(String, String, Option<String>)> {
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("编辑 Snippet"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("保存", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(400, 350);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);

    let folder_combo = gtk::ComboBoxText::new();
    folder_combo.append_text("(根目录)");
    for (_fid, fname) in folders {
        folder_combo.append_text(fname);
    }
    let initial_parent_idx = match current_parent_id {
        Some(pid) => folders
            .iter()
            .position(|(fid, _)| fid == pid)
            .map(|i| i as u32 + 1)
            .unwrap_or(0),
        None => 0,
    };
    folder_combo.set_active(Some(initial_parent_idx));
    let folder_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    folder_row.pack_start(&gtk::Label::new(Some("所属目录：")), false, true, 0);
    folder_row.pack_start(&folder_combo, true, true, 0);
    content.pack_start(&folder_row, false, true, 0);

    let name_label = gtk::Label::new(Some("名称："));
    name_label.set_xalign(0.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_text(existing_name);
    name_entry.set_placeholder_text(Some("片段名称"));
    content.pack_start(&name_label, false, true, 0);
    content.pack_start(&name_entry, false, true, 0);

    let content_label = gtk::Label::new(Some("内容："));
    content_label.set_xalign(0.0);
    content_label.set_margin_top(8);
    let content_view = gtk::TextView::new();
    content_view.buffer().unwrap().set_text(existing_content);
    content_view.set_wrap_mode(gtk::WrapMode::Word);
    let scrolled = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrolled.set_min_content_height(200);
    scrolled.add(&content_view);
    content.pack_start(&content_label, false, true, 0);
    content.pack_start(&scrolled, true, true, 0);

    content.show_all();

    let response = dialog.run();

    let result = if response == gtk::ResponseType::Ok {
        let name = name_entry.text().to_string();
        let buffer = content_view.buffer().unwrap();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .unwrap_or_default()
            .to_string();
        let parent_id = match folder_combo.active() {
            Some(idx) if idx > 0 => folders.get(idx as usize - 1).map(|(id, _)| id.clone()),
            _ => None,
        };

        if name.is_empty() {
            None
        } else {
            Some((name, text, parent_id))
        }
    } else {
        None
    };

    dialog.close();
    result
}

#[cfg(target_os = "linux")]
pub(super) fn show_delete_snippet_dialog(name: &str) -> bool {
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("确认删除"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("删除", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(300, 100);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let message = format!("确定要删除 '{}' 吗？", name);
    let label = gtk::Label::new(Some(&message));
    label.set_xalign(0.5);
    content.pack_start(&label, false, true, 0);

    content.show_all();

    let response = dialog.run();
    dialog.close();

    response == gtk::ResponseType::Ok
}

#[cfg(target_os = "linux")]
pub(super) fn show_edit_folder_dialog(existing_name: &str) -> Option<String> {
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("编辑文件夹"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("保存", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(400, 120);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);

    let name_label = gtk::Label::new(Some("名称："));
    name_label.set_xalign(0.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_text(existing_name);
    name_entry.set_placeholder_text(Some("文件夹名称"));
    content.pack_start(&name_label, false, true, 0);
    content.pack_start(&name_entry, false, true, 0);

    content.show_all();

    let response = dialog.run();
    dialog.close();

    if response == gtk::ResponseType::Ok {
        let name = name_entry.text().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    } else {
        None
    }
}
