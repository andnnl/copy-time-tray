mod dialogs;
mod transfer;
mod tree;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static DRAG_DATA: Mutex<Option<String>> = Mutex::new(None);
static DIALOG_OPEN: AtomicBool = AtomicBool::new(false);

fn new_snippet_parent_id(
    selected_id: &str,
    selected_is_folder: bool,
    selected_item_parent_id: Option<String>,
) -> Option<String> {
    if selected_is_folder {
        Some(selected_id.to_string())
    } else {
        selected_item_parent_id
    }
}

fn is_escape_key(name: &str) -> bool {
    name.eq_ignore_ascii_case("Escape")
}

use dialogs::{
    show_create_snippet_dialog, show_delete_snippet_dialog, show_edit_folder_dialog,
    show_edit_snippet_dialog,
};
use tree::populate_snippet_tree;

#[cfg(target_os = "linux")]
pub fn snippet_dialog_title() -> &'static str {
    "代码片段管理器"
}

#[cfg(target_os = "linux")]
pub fn show() {
    if DIALOG_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }
    use crate::snippet::model::SnippetType;
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some(snippet_dialog_title()),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("关闭", gtk::ResponseType::Close)],
    );
    dialog.set_default_size(800, 550);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();

    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some("搜索片段..."));
    entry.set_margin_bottom(6);
    content.pack_start(&entry, false, true, 4);

    let hpaned = gtk::Paned::new(gtk::Orientation::Horizontal);
    hpaned.set_position(280);

    let left_scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    left_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);

    let store = gtk::TreeStore::new(&[
        String::static_type(), // 0: id
        String::static_type(), // 1: name
        String::static_type(), // 2: content
        bool::static_type(),   // 3: is_folder
        String::static_type(), // 4: icon_name
    ]);

    let snippets = match crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
        Some(guard) => guard.with_store(|s| s.list().to_vec()).unwrap_or_default(),
        None => Vec::new(),
    };

    let folders: Vec<_> = snippets
        .iter()
        .filter(|s| !s.deleted && s.snippet_type == SnippetType::Folder)
        .collect();
    let all_snippets: Vec<_> = snippets
        .iter()
        .filter(|s| !s.deleted && s.snippet_type == SnippetType::Snippet)
        .collect();

    eprintln!("[DEBUG] folders count: {}", folders.len());
    eprintln!("[DEBUG] all_snippets count: {}", all_snippets.len());

    let empty_set = std::collections::HashSet::new();
    let all_folder_ids: std::collections::HashSet<String> =
        folders.iter().map(|f| f.id.clone()).collect();
    populate_snippet_tree(
        &store,
        &folders,
        &all_snippets,
        &None,
        &empty_set,
        &all_folder_ids,
    );
    eprintln!("[DEBUG] tree store populated, total rows will be shown after expand_all");

    let tree_view = gtk::TreeView::with_model(&store);

    let refresh_tree_store = {
        let store = store.clone();
        let tree_view = tree_view.clone();
        move |query: Option<String>, select_id: Option<String>| {
            store.clear();
            let snippets = match crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                Some(guard) => guard.with_store(|s| s.list().to_vec()).unwrap_or_default(),
                None => Vec::new(),
            };
            let all_items: Vec<_> = snippets.iter().filter(|s| !s.deleted).collect();
            let folders: Vec<_> = all_items
                .iter()
                .filter(|s| s.snippet_type == SnippetType::Folder)
                .cloned()
                .collect();
            let all_snippets: Vec<_> = all_items
                .iter()
                .filter(|s| s.snippet_type == SnippetType::Snippet)
                .cloned()
                .collect();

            let q = query.and_then(|q| {
                if q.is_empty() {
                    None
                } else {
                    Some(q.to_lowercase())
                }
            });

            let matching_snippet_ids: std::collections::HashSet<String> =
                if let Some(ref query_lower) = q {
                    all_snippets
                        .iter()
                        .filter(|s| {
                            s.name.to_lowercase().contains(query_lower)
                                || s.content
                                    .as_ref()
                                    .map(|c| c.to_lowercase().contains(query_lower))
                                    .unwrap_or(false)
                        })
                        .map(|s| s.id.clone())
                        .collect()
                } else {
                    std::collections::HashSet::new()
                };

            let folders_to_show: std::collections::HashSet<String> = if let Some(ref query_lower) =
                q
            {
                let mut set = std::collections::HashSet::new();
                let folder_map: std::collections::HashMap<String, &crate::snippet::model::Snippet> =
                    folders.iter().map(|f| (f.id.clone(), *f)).collect();
                for s in &all_snippets {
                    if matching_snippet_ids.contains(&s.id) {
                        let mut current_parent = s.parent_id.clone();
                        while let Some(pid) = current_parent {
                            if set.contains(&pid) {
                                break;
                            }
                            set.insert(pid.clone());
                            current_parent = folder_map.get(&pid).and_then(|f| f.parent_id.clone());
                        }
                    }
                }
                for folder in &folders {
                    if folder.name.to_lowercase().contains(query_lower) {
                        set.insert(folder.id.clone());
                    }
                }
                set
            } else {
                folders.iter().map(|f| f.id.clone()).collect()
            };

            let all_folder_ids: std::collections::HashSet<String> = if q.is_some() {
                folders_to_show.clone()
            } else {
                folders.iter().map(|f| f.id.clone()).collect()
            };

            populate_snippet_tree(
                &store,
                &folders,
                &all_snippets,
                &q,
                &matching_snippet_ids,
                &all_folder_ids,
            );

            tree_view.expand_all();

            if let Some(id) = select_id {
                if let Some(model) = tree_view.model() {
                    if let Some(ts) = model.downcast_ref::<gtk::TreeStore>() {
                        ts.foreach(|model, _path, iter| {
                            let row_id: String =
                                model.value(iter, 0).get::<String>().unwrap_or_default();
                            if row_id == id {
                                tree_view.selection().select_iter(iter);
                                return false;
                            }
                            true
                        });
                    }
                }
            }
        }
    };

    let tree_view_ref = tree_view.clone();
    let search_entry = entry.clone();
    let refresh_for_search = refresh_tree_store.clone();
    search_entry.connect_changed(move |entry| {
        let query = entry.text().to_string();
        refresh_for_search(if query.is_empty() { None } else { Some(query) }, None);
    });
    tree_view.set_headers_visible(false);
    let column = gtk::TreeViewColumn::new();
    let icon_renderer = gtk::CellRendererPixbuf::new();
    let text_renderer = gtk::CellRendererText::new();
    gtk::prelude::TreeViewColumnExt::pack_start(&column, &icon_renderer, false);
    gtk::prelude::TreeViewColumnExt::pack_start(&column, &text_renderer, true);
    gtk::prelude::CellLayoutExt::add_attribute(&column, &icon_renderer, "icon-name", 4);
    gtk::prelude::CellLayoutExt::add_attribute(&column, &text_renderer, "text", 1);
    tree_view.append_column(&column);
    left_scroll.add(&tree_view);

    let right_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let right_label = gtk::Label::new(Some("预览"));
    right_label.set_xalign(0.0);
    right_label.style_context().add_class("dim-label");
    right_box.pack_start(&right_label, false, true, 0);

    let preview_scroll =
        gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    preview_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    let preview_text = gtk::TextView::new();
    preview_text.set_editable(false);
    preview_text.set_cursor_visible(false);
    preview_text.set_wrap_mode(gtk::WrapMode::Word);
    preview_text.set_monospace(true);
    preview_scroll.add(&preview_text);
    right_box.pack_start(&preview_scroll, true, true, 0);

    hpaned.pack1(&left_scroll, true, true);
    hpaned.pack2(&right_box, true, true);
    content.pack_start(&hpaned, true, true, 0);

    let new_button = gtk::Button::with_label("新增 Snippet");
    new_button.style_context().add_class("suggested-action");
    let new_folder_button = gtk::Button::with_label("新增目录");
    let edit_button = gtk::Button::with_label("编辑");
    let delete_button = gtk::Button::with_label("删除");
    delete_button
        .style_context()
        .add_class("destructive-action");
    let copy_button = gtk::Button::with_label("复制");
    copy_button.style_context().add_class("suggested-action");
    copy_button.set_sensitive(false);
    let import_button = gtk::Button::with_label("导入");
    let export_button = gtk::Button::with_label("导出");

    let btn_box = gtk::ButtonBox::new(gtk::Orientation::Horizontal);
    btn_box.set_layout(gtk::ButtonBoxStyle::End);
    btn_box.set_spacing(6);
    btn_box.set_margin_top(6);
    btn_box.pack_start(&import_button, false, true, 0);
    btn_box.pack_start(&export_button, false, true, 0);
    btn_box.pack_start(&new_folder_button, false, true, 0);
    btn_box.pack_start(&new_button, false, true, 0);
    btn_box.pack_start(&edit_button, false, true, 0);
    btn_box.pack_start(&delete_button, false, true, 0);
    btn_box.pack_start(&copy_button, false, true, 0);
    content.pack_start(&btn_box, false, true, 0);

    content.show_all();

    tree_view.expand_all();
    tree_view.set_enable_tree_lines(true);
    tree_view.set_show_expanders(true);

    let target_entry = gtk::TargetEntry::new("UTF8_STRING", gtk::TargetFlags::empty(), 0);
    let drop_tree_view = tree_view.clone();
    tree_view.add_events(
        gtk::gdk::EventMask::BUTTON_PRESS_MASK
            | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
            | gtk::gdk::EventMask::BUTTON_MOTION_MASK
            | gtk::gdk::EventMask::POINTER_MOTION_MASK,
    );
    tree_view.drag_source_set(
        gtk::gdk::ModifierType::BUTTON1_MASK,
        &[target_entry.clone()],
        gtk::gdk::DragAction::MOVE,
    );
    tree_view.drag_source_set_icon_name("text-x-script-symbolic");
    tree_view.connect_drag_begin(move |_, drag_context| {
        eprintln!("[DEBUG] drag_begin called, context: {:?}", drag_context);
    });
    tree_view.connect_drag_data_get(move |_, _, data, info, _| {
        eprintln!("[DEBUG] drag_data_get called: info={}", info);
        if info == 0 {
            let tv = drop_tree_view.clone();
            if let Some(sel) = tv.selection().selected() {
                let (model, iter) = sel;
                if let Some(ts) = model.downcast_ref::<gtk::TreeStore>() {
                    let is_folder: bool = ts.value(&iter, 3).get::<bool>().unwrap_or_default();
                    eprintln!("[DEBUG] drag_data_get: is_folder={}", is_folder);
                    if is_folder {
                        return;
                    }
                    let id: String = ts.value(&iter, 0).get::<String>().unwrap_or_default();
                    eprintln!("[DEBUG] drag_data_get: storing id '{}' in DRAG_DATA", id);
                    if let Ok(mut drag_data) = DRAG_DATA.lock() {
                        *drag_data = Some(id);
                    }
                }
            }
        }
    });
    let dest_tree_view = tree_view.clone();
    let refresh_for_drag = refresh_tree_store.clone();
    tree_view.drag_dest_set(
        gtk::DestDefaults::empty(),
        &[target_entry],
        gtk::gdk::DragAction::MOVE,
    );
    tree_view.connect_drag_drop(move |tv, dc, _x, _y, time| {
        eprintln!("[DEBUG] drag_drop called");
        let target = tv.drag_dest_find_target(dc, None);
        eprintln!("[DEBUG] drag_drop: found target = {:?}", target.is_some());
        if let Some(target) = target {
            tv.drag_get_data(dc, &target, time);
            true
        } else {
            false
        }
    });
    tree_view.connect_drag_motion(move |_tv, dc, _x, _y, time| {
        dc.drag_status(gtk::gdk::DragAction::MOVE, time);
        true
    });
    tree_view.connect_drag_data_received(move |tv, _dc, x, y, _data, _info, _| {
        eprintln!("[DEBUG] drag_data_received: x={}, y={}", x, y);
        let dragged_id = match DRAG_DATA.lock().ok().and_then(|mut data| data.take()) {
            Some(id) => id,
            None => {
                eprintln!("[DEBUG] drag_data_received: DRAG_DATA is empty");
                return;
            }
        };
        eprintln!("[DEBUG] dragged_id={}", dragged_id);
        let drop_opt = dest_tree_view.dest_row_at_pos(x, y);
        eprintln!("[DEBUG] drop_opt present = {}", drop_opt.is_some());
        match drop_opt {
            Some((Some(ref path), gtk::TreeViewDropPosition::IntoOrBefore))
            | Some((Some(ref path), gtk::TreeViewDropPosition::IntoOrAfter)) => {
                let model = dest_tree_view.model().unwrap();
                let ts = match model.downcast_ref::<gtk::TreeStore>() {
                    Some(m) => m,
                    None => return,
                };
                if let Some(iter) = ts.iter(&path) {
                    let target_is_folder: bool =
                        ts.value(&iter, 3).get::<bool>().unwrap_or_default();
                    if !target_is_folder {
                        return;
                    }
                    let target_id: String = ts.value(&iter, 0).get::<String>().unwrap_or_default();
                    eprintln!("[DEBUG] moving {} into folder {}", dragged_id, target_id);
                    if let Some(manager) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                        let _ = manager.with_store_mut(|s| {
                            let _ = s.update(&dragged_id, |snip| {
                                snip.parent_id = Some(target_id);
                            });
                        });
                    }
                    refresh_for_drag(None, Some(dragged_id.clone()));
                }
            }
            _ => {
                eprintln!("[DEBUG] moving {} to root", dragged_id);
                if let Some(manager) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                    let _ = manager.with_store_mut(|s| {
                        let _ = s.update(&dragged_id, |snip| {
                            snip.parent_id = None;
                        });
                    });
                }
                refresh_for_drag(None, Some(dragged_id.clone()));
            }
        }
    });

    let selection = tree_view.selection();
    selection.set_mode(gtk::SelectionMode::Single);

    let preview_text_ref = preview_text.clone();
    let copy_button_ref = copy_button.clone();
    let selected_content: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_id: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_is_folder: std::sync::Arc<std::sync::Mutex<Option<bool>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_name: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_parent_id: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));

    let sc_clone = selected_content.clone();
    let si_clone = selected_id.clone();
    let sf_clone = selected_is_folder.clone();
    let sn_clone = selected_name.clone();
    let sp_clone = selected_parent_id.clone();
    selection.connect_changed(move |sel| {
        if let Some((model, iter)) = sel.selected() {
            if let Some(tree_store) = model.downcast_ref::<gtk::TreeStore>() {
                let id: String = tree_store
                    .value(&iter, 0)
                    .get::<String>()
                    .unwrap_or_default();
                let name: String = tree_store
                    .value(&iter, 1)
                    .get::<String>()
                    .unwrap_or_default();
                let content: String = tree_store
                    .value(&iter, 2)
                    .get::<String>()
                    .unwrap_or_default();
                let is_folder: bool = tree_store.value(&iter, 3).get::<bool>().unwrap_or_default();
                let selected_item_parent_id = if is_folder {
                    None
                } else {
                    crate::app::MANAGER
                        .get()
                        .and_then(|manager| manager.lock().ok())
                        .and_then(|manager| {
                            manager
                                .with_store(|store| {
                                    store.get(&id).and_then(|snippet| snippet.parent_id.clone())
                                })
                                .ok()
                                .flatten()
                        })
                };
                let parent_id = new_snippet_parent_id(&id, is_folder, selected_item_parent_id);
                *sc_clone.lock().unwrap() = Some(content.clone());
                *si_clone.lock().unwrap() = Some(id);
                *sf_clone.lock().unwrap() = Some(is_folder);
                *sn_clone.lock().unwrap() = Some(name);
                *sp_clone.lock().unwrap() = parent_id;
                if is_folder {
                    if let Some(buffer) = preview_text_ref.buffer() {
                        buffer.set_text("(文件夹)");
                    }
                    copy_button_ref.set_sensitive(false);
                } else {
                    if let Some(buffer) = preview_text_ref.buffer() {
                        buffer.set_text(&content);
                    }
                    copy_button_ref.set_sensitive(true);
                }
            }
        }
    });

    let dialog_for_dblclick = dialog.clone();
    tree_view.connect_row_activated(move |tv, path, _col| {
        if let Some(model) = tv.model() {
            if let Some(ts) = model.downcast_ref::<gtk::TreeStore>() {
                if let Some(iter) = ts.iter(path) {
                    let is_folder: bool = ts.value(&iter, 3).get::<bool>().unwrap_or_default();
                    if is_folder {
                        if tv.row_expanded(path) {
                            tv.collapse_row(path);
                        } else {
                            tv.expand_row(path, false);
                        }
                    } else {
                        let content: String =
                            ts.value(&iter, 2).get::<String>().unwrap_or_default();
                        let name: String = ts.value(&iter, 1).get::<String>().unwrap_or_default();
                        if !content.is_empty() {
                            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
                            clipboard.set_text(&content);
                            clipboard.store();
                            let _ = notify_rust::Notification::new()
                                .summary("Snippet Copied")
                                .body(&format!(
                                    "已复制: {}",
                                    name.chars().take(30).collect::<String>()
                                ))
                                .show();
                            if let Some(d) = dialog_for_dblclick.downcast_ref::<gtk::Dialog>() {
                                d.response(gtk::ResponseType::Close);
                            }
                        }
                    }
                }
            }
        }
    });

    let dialog_for_copy = dialog.clone();
    let sc_for_copy = selected_content.clone();
    copy_button.connect_clicked(move |_| {
        let content = sc_for_copy.lock().unwrap().clone();
        if let Some(c) = content {
            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
            clipboard.set_text(&c);
            clipboard.store();
            let _ = notify_rust::Notification::new()
                .summary("Snippet Copied")
                .body(&format!(
                    "已复制: {}",
                    c.chars().take(30).collect::<String>()
                ))
                .show();
            if let Some(d) = dialog_for_copy.downcast_ref::<gtk::Dialog>() {
                d.response(gtk::ResponseType::Close);
            }
        }
    });

    let dialog_for_new_folder = dialog.clone();
    new_folder_button.connect_clicked(move |_| {
        eprintln!("[DEBUG] new_folder_button clicked");
        if let Some(d) = dialog_for_new_folder.downcast_ref::<gtk::Dialog>() {
            eprintln!("[DEBUG] sending Accept response for new folder");
            d.response(gtk::ResponseType::Accept);
        } else {
            eprintln!("[DEBUG] failed to downcast dialog for new folder");
        }
    });

    let dialog_for_new_snippet = dialog.clone();
    new_button.connect_clicked(move |_| {
        if let Some(d) = dialog_for_new_snippet.downcast_ref::<gtk::Dialog>() {
            d.response(gtk::ResponseType::Apply);
        }
    });

    let dialog_for_edit = dialog.clone();
    let si_for_edit = selected_id.clone();
    let sf_for_edit = selected_is_folder.clone();
    let sn_for_edit = selected_name.clone();
    let sc_for_edit = selected_content.clone();
    let refresh_for_edit = refresh_tree_store.clone();
    edit_button.connect_clicked(move |_| {
        let is_folder = sf_for_edit.lock().unwrap().unwrap_or(false);
        let id_opt = si_for_edit.lock().unwrap().clone();
        let name_opt = sn_for_edit.lock().unwrap().clone();
        if let Some(_id) = id_opt {
            let name = name_opt.unwrap_or_default();
            if is_folder {
                eprintln!("[DEBUG] editing folder: {}", _id);
                if let Some(new_name) = show_edit_folder_dialog(&name) {
                    {
                        if let Some(manager_guard) =
                            crate::app::MANAGER.get().and_then(|m| m.lock().ok())
                        {
                            let manager = &*manager_guard;
                            let _ = manager.with_store_mut(|store| {
                                store.update(&_id, |s| {
                                    s.name = new_name.clone();
                                })
                            });
                        }
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("文件夹已更新")
                        .body(&format!("'{}' 已重命名", new_name))
                        .show();
                    refresh_for_edit(None, Some(_id.clone()));
                }
            } else {
                eprintln!("[DEBUG] editing snippet: {}", _id);
                let content = sc_for_edit.lock().unwrap().clone().unwrap_or_default();
                let folders_data: Vec<(String, String)> =
                    if let Some(mg) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                        mg.with_store(|s| {
                            s.list()
                                .iter()
                                .filter(|sn| !sn.deleted && sn.snippet_type == SnippetType::Folder)
                                .map(|sn| (sn.id.clone(), sn.name.clone()))
                                .collect()
                        })
                        .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                let parent_id_opt: Option<String> =
                    if let Some(mg) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                        mg.with_store(|s| s.get(&_id).map(|sn| sn.parent_id.clone()).flatten())
                            .ok()
                            .flatten()
                    } else {
                        None
                    };
                if let Some((new_name, new_content, new_parent_id)) = show_edit_snippet_dialog(
                    &name,
                    &content,
                    &folders_data,
                    parent_id_opt.as_deref(),
                ) {
                    {
                        if let Some(manager_guard) =
                            crate::app::MANAGER.get().and_then(|m| m.lock().ok())
                        {
                            let manager = &*manager_guard;
                            let _ = manager.with_store_mut(|store| {
                                store.update(&_id, |s| {
                                    s.name = new_name.clone();
                                    s.content = Some(new_content.clone());
                                    s.parent_id = new_parent_id.clone();
                                })
                            });
                        }
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("Snippet 已更新")
                        .body(&format!("'{}' 已保存", new_name))
                        .show();
                    refresh_for_edit(None, Some(_id.clone()));
                }
            }
        }
    });

    let dialog_for_del = dialog.clone();
    let si_for_del = selected_id.clone();
    let sn_for_del = selected_name.clone();
    let sf_for_del = selected_is_folder.clone();
    let refresh_for_del = refresh_tree_store.clone();
    delete_button.connect_clicked(move |_| {
        let is_folder = sf_for_del.lock().unwrap().unwrap_or(false);
        let id_opt = si_for_del.lock().unwrap().clone();
        let name_opt = sn_for_del.lock().unwrap().clone();
        if let (Some(id), Some(name)) = (id_opt, name_opt) {
            if show_delete_snippet_dialog(&name) {
                {
                    if let Some(manager_guard) =
                        crate::app::MANAGER.get().and_then(|m| m.lock().ok())
                    {
                        let manager = &*manager_guard;
                        let _ = manager.with_store_mut(|store| store.delete(&id));
                    }
                }
                let _ = notify_rust::Notification::new()
                    .summary("Snippet 已删除")
                    .body(&format!("'{}' 已删除", name))
                    .show();
                refresh_for_del(None, None);
            }
        }
    });

    // GTK selects the file and hands only its path to a pure-data worker.
    let dialog_for_import = dialog.clone();
    let refresh_for_import = refresh_tree_store.clone();
    import_button.connect_clicked(move |button| {
        let file_chooser = gtk::FileChooserDialog::with_buttons::<gtk::Window>(
            Some("导入代码片段"),
            Some(dialog_for_import.upcast_ref::<gtk::Window>()),
            gtk::FileChooserAction::Open,
            &[
                ("取消", gtk::ResponseType::Cancel),
                ("导入", gtk::ResponseType::Accept),
            ],
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("JSON 文件"));
        filter.add_pattern("*.json");
        file_chooser.add_filter(filter);
        let file_path = if file_chooser.run() == gtk::ResponseType::Accept {
            file_chooser.filename()
        } else {
            None
        };
        file_chooser.close();

        if let Some(path) = file_path {
            button.set_sensitive(false);
            let (sender, receiver) = async_channel::bounded(1);
            transfer::spawn_import(path, sender);
            let button = button.clone();
            let refresh = refresh_for_import.clone();
            gtk::glib::spawn_future_local(async move {
                match receiver.recv().await {
                    Ok(transfer::ImportResult::Imported(Ok(summary))) => {
                        if summary.added > 0 {
                            refresh(None, None);
                        }
                        let _ = notify_rust::Notification::new()
                            .summary("导入完成")
                            .body(&format!(
                                "新增 {} 条，跳过 {} 条相同 ID，跳过 {} 条重名",
                                summary.added,
                                summary.skipped_existing_id,
                                summary.skipped_duplicate
                            ))
                            .show();
                    }
                    Ok(transfer::ImportResult::Imported(Err(error))) => {
                        let _ = notify_rust::Notification::new()
                            .summary("导入失败")
                            .body(&error)
                            .show();
                    }
                    Err(error) => {
                        let _ = notify_rust::Notification::new()
                            .summary("导入失败")
                            .body(&error.to_string())
                            .show();
                    }
                }
                button.set_sensitive(true);
            });
        }
    });

    // The GTK thread only selects a destination and captures an in-memory snapshot.
    // JSON conversion and disk I/O run on a worker to keep the dialog responsive.
    let dialog_for_export = dialog.clone();
    export_button.connect_clicked(move |button| {
        let file_chooser = gtk::FileChooserDialog::with_buttons::<gtk::Window>(
            Some("导出代码片段"),
            Some(dialog_for_export.upcast_ref::<gtk::Window>()),
            gtk::FileChooserAction::Save,
            &[
                ("取消", gtk::ResponseType::Cancel),
                ("导出", gtk::ResponseType::Accept),
            ],
        );
        file_chooser.set_current_name("snippets.json");
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("JSON 文件"));
        filter.add_pattern("*.json");
        file_chooser.add_filter(filter);
        let file_path = if file_chooser.run() == gtk::ResponseType::Accept {
            file_chooser.filename()
        } else {
            None
        };
        file_chooser.close();

        if let Some(path) = file_path {
            let snippets = match crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                Some(manager) => manager
                    .with_store(|store| store.list().to_vec())
                    .unwrap_or_default(),
                None => {
                    let _ = notify_rust::Notification::new()
                        .summary("导出失败")
                        .body("Snippet 管理器未初始化")
                        .show();
                    return;
                }
            };

            button.set_sensitive(false);
            let (sender, receiver) = async_channel::bounded(1);
            transfer::spawn_export(snippets, path.clone(), sender);
            let button = button.clone();
            gtk::glib::spawn_future_local(async move {
                match receiver.recv().await {
                    Ok(transfer::ExportResult::Exported(Ok(()))) => {
                        let _ = notify_rust::Notification::new()
                            .summary("导出成功")
                            .body(&format!("已导出到 {}", path.display()))
                            .show();
                    }
                    Ok(transfer::ExportResult::Exported(Err(error))) => {
                        let _ = notify_rust::Notification::new()
                            .summary("导出失败")
                            .body(&error)
                            .show();
                    }
                    Err(error) => {
                        let _ = notify_rust::Notification::new()
                            .summary("导出失败")
                            .body(&error.to_string())
                            .show();
                    }
                }
                button.set_sensitive(true);
            });
        }
    });

    let dialog_for_escape = dialog.clone();
    dialog.connect_key_press_event(move |_, event| {
        let key_name = event
            .keyval()
            .name()
            .map(|name| name.to_string())
            .unwrap_or_default();
        if is_escape_key(&key_name) {
            dialog_for_escape.hide();
            DIALOG_OPEN.store(false, Ordering::SeqCst);
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });

    dialog.show_all();
    entry.grab_focus();
    let refresh = refresh_tree_store.clone();
    let dialog_for_response = dialog.clone();
    let selected_parent_id_for_create = selected_parent_id.clone();
    dialog.connect_response(move |_, response| match response {
        gtk::ResponseType::Accept => {
            eprintln!("[DEBUG] Accept response");
            let name_input = gtk::Dialog::with_buttons::<gtk::Window>(
                Some("新增目录"),
                Some(dialog_for_response.upcast_ref::<gtk::Window>()),
                gtk::DialogFlags::MODAL,
                &[
                    ("取消", gtk::ResponseType::Cancel),
                    ("创建", gtk::ResponseType::Ok),
                ],
            );
            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("目录名称"));
            let ca = name_input.content_area();
            ca.set_margin_start(10);
            ca.set_margin_end(10);
            ca.add(&entry);
            ca.show_all();
            let folder_response = name_input.run();
            eprintln!("[DEBUG] new folder dialog response: {:?}", folder_response);
            if folder_response == gtk::ResponseType::Ok {
                let folder_name = entry.text().to_string();
                eprintln!("[DEBUG] creating folder: '{}'", folder_name);
                if !folder_name.is_empty() {
                    if let Some(mg) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                        let manager = &*mg;
                        let folder =
                            crate::snippet::model::Snippet::new_folder(folder_name.clone(), None);
                        if let Err(e) = manager.with_store_mut(|store| store.add(folder)) {
                            let _ = notify_rust::Notification::new()
                                .summary("创建失败")
                                .body(&format!("{}", e))
                                .show();
                        } else {
                            let _ = notify_rust::Notification::new()
                                .summary("目录已创建")
                                .body(&format!("'{}' 已保存", folder_name))
                                .show();
                        }
                    }
                }
            }
            name_input.close();
            refresh(None, None);
        }
        gtk::ResponseType::Apply => {
            eprintln!("[DEBUG] Apply response");
            let folder_data: Vec<(String, String)> = {
                if let Some(mg) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                    mg.with_store(|s| {
                        s.list()
                            .iter()
                            .filter(|sn| !sn.deleted && sn.snippet_type == SnippetType::Folder)
                            .map(|sn| (sn.id.clone(), sn.name.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
                } else {
                    Vec::new()
                }
            };
            if let Some((name, text_content, parent_id)) = show_create_snippet_dialog(
                &folder_data,
                selected_parent_id_for_create.lock().unwrap().as_deref(),
            ) {
                if let Some(mg) = crate::app::MANAGER.get().and_then(|m| m.lock().ok()) {
                    let manager = &*mg;
                    let snippet = crate::snippet::model::Snippet::new_snippet(
                        name.clone(),
                        text_content,
                        parent_id,
                    );
                    if let Err(e) = manager.with_store_mut(|store| store.add(snippet)) {
                        let _ = notify_rust::Notification::new()
                            .summary("创建失败")
                            .body(&format!("{}", e))
                            .show();
                    } else {
                        let _ = notify_rust::Notification::new()
                            .summary("Snippet 已创建")
                            .body(&format!("'{}' 已保存", name))
                            .show();
                    }
                }
            }
            refresh(None, None);
        }
        gtk::ResponseType::Close | gtk::ResponseType::DeleteEvent => {
            eprintln!("[DEBUG] Close response");
            dialog_for_response.hide();
            DIALOG_OPEN.store(false, Ordering::SeqCst);
        }
        _ => {
            eprintln!("[DEBUG] Unhandled response: {:?}", response);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{is_escape_key, new_snippet_parent_id};

    #[test]
    fn only_escape_hides_the_main_snippet_dialog() {
        assert!(is_escape_key("Escape"));
        assert!(!is_escape_key("Return"));
    }

    #[test]
    fn selected_folder_is_the_default_parent_for_new_snippets() {
        assert_eq!(
            new_snippet_parent_id("folder-id", true, None),
            Some("folder-id".to_string())
        );
    }

    #[test]
    fn selected_snippet_uses_its_stored_parent_as_the_default() {
        assert_eq!(
            new_snippet_parent_id("snippet-id", false, Some("folder-id".to_string())),
            Some("folder-id".to_string())
        );
    }

    #[test]
    fn root_snippet_has_no_default_parent_for_new_snippets() {
        assert_eq!(new_snippet_parent_id("snippet-id", false, None), None);
    }
}
