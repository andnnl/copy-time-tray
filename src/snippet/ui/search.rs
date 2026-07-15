use crate::snippet::model::Snippet;
use crate::snippet::ui::editor::EditorPopup;
use crate::snippet::ui::{UiCommand, UiResponse};
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use eframe::egui;

pub struct SearchPopup {
    search_query: String,
    filtered: Vec<Snippet>,
    selected_index: usize,
    snippets: Vec<Snippet>,
    expanded_folders: Vec<String>,
    visible: bool,
    last_query: String,
    cmd_receiver: Option<Receiver<UiCommand>>,
    resp_sender: Option<Sender<UiResponse>>,
    editor: EditorPopup,
    search_focus_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowVisibilityAction {
    ShowAndFocus,
    Hide,
}

impl Default for SearchPopup {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            filtered: Vec::new(),
            selected_index: 0,
            snippets: Vec::new(),
            expanded_folders: Vec::new(),
            visible: true,
            last_query: String::new(),
            cmd_receiver: None,
            resp_sender: None,
            editor: EditorPopup::default(),
            search_focus_requested: true,
        }
    }
}

impl SearchPopup {
    pub fn new(cmd_receiver: Receiver<UiCommand>, resp_sender: Sender<UiResponse>) -> Self {
        Self {
            cmd_receiver: Some(cmd_receiver),
            resp_sender: Some(resp_sender.clone()),
            editor: EditorPopup::new(resp_sender),
            ..Default::default()
        }
    }

    fn filter_snippets(&mut self) {
        if self.search_query.is_empty() {
            self.filtered = self.snippets.clone();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered = self
                .snippets
                .iter()
                .filter(|s| {
                    s.name.to_lowercase().contains(&query)
                        || s.content
                            .as_ref()
                            .map(|c| c.to_lowercase().contains(&query))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();
        }
        if self.selected_index >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected_index = self.filtered.len() - 1;
        }
    }

    fn show_window(&mut self) -> WindowVisibilityAction {
        self.visible = true;
        self.search_focus_requested = true;
        WindowVisibilityAction::ShowAndFocus
    }

    fn hide_window(&mut self) -> WindowVisibilityAction {
        self.visible = false;
        WindowVisibilityAction::Hide
    }

    fn take_search_focus_request(&mut self) -> bool {
        std::mem::take(&mut self.search_focus_requested)
    }

    fn apply_window_action(ctx: &egui::Context, action: WindowVisibilityAction) {
        match action {
            WindowVisibilityAction::ShowAndFocus => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            WindowVisibilityAction::Hide => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
    }

    fn process_commands(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.cmd_receiver.take() {
            loop {
                match rx.try_recv() {
                    Ok(UiCommand::Show) => {
                        let action = self.show_window();
                        Self::apply_window_action(ctx, action);
                    }
                    Ok(UiCommand::Hide) => {
                        let action = self.hide_window();
                        Self::apply_window_action(ctx, action);
                    }
                    Ok(UiCommand::RefreshSnippets(snippets)) => {
                        self.snippets = snippets;
                        self.filter_snippets();
                    }
                    Ok(UiCommand::CopyToClipboard(_)) => {}
                    Ok(UiCommand::Sync) => {}
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
            self.cmd_receiver = Some(rx);
        }
        if self.visible {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

impl eframe::App for SearchPopup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_commands(ctx);

        if !self.visible {
            return;
        }

        if self.editor.is_visible() {
            self.editor.update(ctx, _frame);
            return;
        }

        egui::Window::new("Snippet 搜索")
            .collapsible(false)
            .resizable(false)
            .default_width(600.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                if ui.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl) {
                    self.editor.show_create();
                }

                if ui.input(|i| i.key_pressed(egui::Key::E) && i.modifiers.ctrl) {
                    if let Some(snippet) = self.filtered.get(self.selected_index) {
                        if matches!(
                            snippet.snippet_type,
                            crate::snippet::model::SnippetType::Snippet
                        ) {
                            self.editor.show_edit(snippet.clone());
                        }
                    }
                }

                if ui.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
                    if let Some(snippet) = self.filtered.get(self.selected_index) {
                        if matches!(
                            snippet.snippet_type,
                            crate::snippet::model::SnippetType::Snippet
                        ) {
                            self.editor.show_delete(snippet.clone());
                        }
                    }
                }

                let focus_search = self.take_search_focus_request();
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    let response = ui.text_edit_singleline(&mut self.search_query);
                    if focus_search {
                        response.request_focus();
                    }
                });

                if self.search_query != self.last_query {
                    self.filter_snippets();
                    self.last_query = self.search_query.clone();
                }

                ui.separator();

                let mut copy_index = None;
                let _scroll = egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, snippet) in self.filtered.iter().enumerate() {
                        let is_selected = i == self.selected_index;
                        let prefix = match snippet.snippet_type {
                            crate::snippet::model::SnippetType::Folder => "📁",
                            crate::snippet::model::SnippetType::Snippet => "📄",
                        };
                        let content_preview = snippet
                            .content
                            .as_deref()
                            .unwrap_or("")
                            .chars()
                            .take(30)
                            .collect::<String>();

                        let bg = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            ui.visuals().widgets.noninteractive.weak_bg_fill
                        };

                        ui.painter()
                            .rect_filled(ui.available_rect_before_wrap(), 2.0, bg);

                        ui.colored_label(
                            if is_selected {
                                ui.visuals().selection.stroke.color
                            } else {
                                ui.visuals().widgets.noninteractive.fg_stroke.color
                            },
                            format!("{} {}", prefix, snippet.name),
                        );

                        if !content_preview.is_empty() {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(&content_preview)
                                            .color(
                                                ui.visuals().widgets.noninteractive.fg_stroke.color,
                                            )
                                            .small(),
                                    );
                                },
                            );
                        }

                        if is_selected && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            match snippet.snippet_type {
                                crate::snippet::model::SnippetType::Folder => {
                                    let folder_id = snippet.id.clone();
                                    if self.expanded_folders.contains(&folder_id) {
                                        self.expanded_folders.retain(|id| id != &folder_id);
                                    } else {
                                        self.expanded_folders.push(folder_id);
                                    }
                                }
                                crate::snippet::model::SnippetType::Snippet => {}
                            }
                        }

                        if is_selected
                            && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl)
                        {
                            if let Some(ref content) = snippet.content {
                                copy_index = Some(content.clone());
                            }
                        }
                    }
                });

                ui.separator();

                if let Some(content) = copy_index {
                    if let Some(ref sender) = self.resp_sender {
                        let _ = sender.send(UiResponse::SnippetCopied(content));
                    }
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && self.selected_index > 0 {
                    self.selected_index -= 1;
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                    && self.selected_index + 1 < self.filtered.len()
                {
                    self.selected_index += 1;
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    let action = self.hide_window();
                    if let Some(ref sender) = self.resp_sender {
                        let _ = sender.send(UiResponse::Close);
                    }
                    Self::apply_window_action(ctx, action);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::{SearchPopup, WindowVisibilityAction};

    #[test]
    fn show_requests_window_visibility_focus_and_search_focus() {
        let mut popup = SearchPopup::default();

        let action = popup.show_window();

        assert_eq!(action, WindowVisibilityAction::ShowAndFocus);
        assert!(popup.visible);
        assert!(popup.take_search_focus_request());
        assert!(!popup.take_search_focus_request());
    }

    #[test]
    fn escape_hides_without_closing_the_native_window() {
        let mut popup = SearchPopup::default();

        let action = popup.hide_window();

        assert_eq!(action, WindowVisibilityAction::Hide);
        assert!(!popup.visible);
    }
}
