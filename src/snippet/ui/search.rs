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
    maximized: bool,
    last_saved_window_state: Option<crate::snippet::config::SnippetWindowState>,
    pending_window_state: Option<crate::snippet::config::SnippetWindowState>,
    window_state_changed_at: Option<std::time::Instant>,
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
            maximized: false,
            last_saved_window_state: None,
            pending_window_state: None,
            window_state_changed_at: None,
        }
    }
}

impl SearchPopup {
    pub fn new(cmd_receiver: Receiver<UiCommand>, resp_sender: Sender<UiResponse>) -> Self {
        let window_state = crate::snippet::config::SnippetWindowState::load().unwrap_or_default();
        Self {
            cmd_receiver: Some(cmd_receiver),
            resp_sender: Some(resp_sender.clone()),
            editor: EditorPopup::new(resp_sender),
            maximized: window_state.maximized,
            last_saved_window_state: Some(window_state),
            ..Default::default()
        }
    }

    fn current_window_state(ctx: &egui::Context) -> crate::snippet::config::SnippetWindowState {
        ctx.input(|input| {
            let viewport = input.viewport();
            let inner = viewport.inner_rect;
            let outer = viewport.outer_rect;
            crate::snippet::config::SnippetWindowState {
                width: inner.map(|rect| rect.width().round() as i32).unwrap_or(800),
                height: inner
                    .map(|rect| rect.height().round() as i32)
                    .unwrap_or(550),
                x: outer.map(|rect| rect.min.x.round() as i32),
                y: outer.map(|rect| rect.min.y.round() as i32),
                maximized: viewport.maximized.unwrap_or(false),
            }
        })
    }

    fn persist_window_state(&mut self, ctx: &egui::Context, force: bool) {
        let mut state = Self::current_window_state(ctx);
        if state.maximized {
            if let Some(previous) = &self.last_saved_window_state {
                state.width = previous.width;
                state.height = previous.height;
                state.x = previous.x;
                state.y = previous.y;
            }
        }
        self.maximized = state.maximized;
        if self.last_saved_window_state.as_ref() == Some(&state) {
            self.pending_window_state = None;
            self.window_state_changed_at = None;
            return;
        }
        if self.pending_window_state.as_ref() != Some(&state) {
            self.pending_window_state = Some(state.clone());
            self.window_state_changed_at = Some(std::time::Instant::now());
        }
        let settled = self
            .window_state_changed_at
            .map(|changed_at| changed_at.elapsed() >= std::time::Duration::from_millis(400))
            .unwrap_or(false);
        if !force && !settled {
            return;
        }
        let state = self.pending_window_state.take().unwrap_or(state);
        if let Err(error) = state.save() {
            eprintln!("[WARN] failed to save snippet window state: {error}");
            return;
        }
        self.window_state_changed_at = None;
        self.last_saved_window_state = Some(state);
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
                        self.persist_window_state(ctx, true);
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
        self.persist_window_state(ctx, false);

        if !self.visible {
            return;
        }

        if self.editor.is_visible() {
            self.editor.update(ctx, _frame);
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("代码片段");
            ui.separator();
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

            let favorites: Vec<Snippet> = self
                .snippets
                .iter()
                .filter(|snippet| {
                    !snippet.deleted
                        && snippet.favorite
                        && snippet.snippet_type == crate::snippet::model::SnippetType::Snippet
                })
                .cloned()
                .collect();
            let mut favorite_toggle: Option<(String, bool)> = None;
            if !favorites.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for favorite in &favorites {
                        let response = ui.add(egui::Button::new(format!("★ {}", favorite.name)));
                        if response.clicked() {
                            if let Some(sender) = &self.resp_sender {
                                let _ = sender.send(UiResponse::SnippetCopied(
                                    favorite.content.clone().unwrap_or_default(),
                                ));
                            }
                        }
                        response.context_menu(|ui| {
                            if ui.button("取消收藏").clicked() {
                                favorite_toggle = Some((favorite.id.clone(), false));
                                ui.close_menu();
                            }
                        });
                    }
                });
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

                    let response =
                        ui.selectable_label(is_selected, format!("{} {}", prefix, snippet.name));
                    if response.clicked() {
                        self.selected_index = i;
                    }
                    if snippet.snippet_type == crate::snippet::model::SnippetType::Snippet {
                        response.context_menu(|ui| {
                            let label = if snippet.favorite {
                                "取消收藏"
                            } else {
                                "收藏"
                            };
                            if ui.button(label).clicked() {
                                favorite_toggle = Some((snippet.id.clone(), !snippet.favorite));
                                ui.close_menu();
                            }
                        });
                    }

                    if !content_preview.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(&content_preview)
                                    .color(ui.visuals().widgets.noninteractive.fg_stroke.color)
                                    .small(),
                            );
                        });
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

            if let Some((id, favorite)) = favorite_toggle {
                if let Some(sender) = &self.resp_sender {
                    let _ = sender.send(UiResponse::FavoriteToggled { id, favorite });
                }
            }

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
                self.persist_window_state(ctx, true);
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
