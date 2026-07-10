use crate::snippet::model::Snippet;
use crate::snippet::ui::UiResponse;
use crossbeam_channel::Sender;
use eframe::egui;

#[derive(Debug, Clone)]
pub enum EditorAction {
    Create {
        name: String,
        content: String,
        parent_id: Option<String>,
    },
    Update {
        id: String,
        name: String,
        content: String,
    },
    Delete {
        id: String,
    },
    Cancel,
}

pub struct EditorPopup {
    mode: EditorMode,
    name: String,
    content: String,
    selected_parent: Option<String>,
    show: bool,
    resp_sender: Option<Sender<UiResponse>>,
}

#[derive(Debug, Clone)]
enum EditorMode {
    Create,
    Edit(Snippet),
    DeleteConfirm(Snippet),
}

impl Default for EditorPopup {
    fn default() -> Self {
        Self {
            mode: EditorMode::Create,
            name: String::new(),
            content: String::new(),
            selected_parent: None,
            show: false,
            resp_sender: None,
        }
    }
}

impl EditorPopup {
    pub fn new(resp_sender: Sender<UiResponse>) -> Self {
        Self {
            resp_sender: Some(resp_sender),
            ..Default::default()
        }
    }

    pub fn show_create(&mut self) {
        self.mode = EditorMode::Create;
        self.name.clear();
        self.content.clear();
        self.selected_parent = None;
        self.show = true;
    }

    pub fn show_edit(&mut self, snippet: Snippet) {
        self.mode = EditorMode::Edit(snippet.clone());
        self.name = snippet.name.clone();
        self.content = snippet.content.clone().unwrap_or_default();
        self.selected_parent = snippet.parent_id.clone();
        self.show = true;
    }

    pub fn show_delete(&mut self, snippet: Snippet) {
        self.mode = EditorMode::DeleteConfirm(snippet.clone());
        self.name = snippet.name.clone();
        self.show = true;
    }

    pub fn is_visible(&self) -> bool {
        self.show
    }

    pub fn hide(&mut self) {
        self.show = false;
    }
}

impl eframe::App for EditorPopup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.show {
            return;
        }

        let title = match &self.mode {
            EditorMode::Create => "新建 Snippet",
            EditorMode::Edit(_) => "编辑 Snippet",
            EditorMode::DeleteConfirm(_) => "确认删除",
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(400.0)
            .default_height(300.0)
            .show(ctx, |ui| match &self.mode {
                EditorMode::DeleteConfirm(snippet) => {
                    let name = snippet.name.clone();
                    let id = snippet.id.clone();
                    ui.label(format!("确定要删除 \"{}\" 吗？", name));
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        if ui.button("取消").clicked() {
                            self.hide();
                        }
                        if ui.button("删除").clicked() {
                            if let Some(ref sender) = self.resp_sender {
                                let _ = sender.send(UiResponse::SnippetDeleted(id));
                            }
                            self.hide();
                        }
                    });
                }
                _ => {
                    let is_create = matches!(self.mode, EditorMode::Create);
                    let edit_snippet = if let EditorMode::Edit(s) = &self.mode {
                        Some(s.clone())
                    } else {
                        None
                    };

                    ui.label("名称:");
                    ui.text_edit_singleline(&mut self.name);
                    ui.add_space(4.0);

                    ui.label("文件夹:");
                    ui.label("(暂不支持选择父文件夹)");
                    ui.add_space(4.0);

                    ui.label("内容:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.content)
                            .desired_rows(6)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui.button("取消").clicked() {
                            self.hide();
                        }
                        if ui.button("保存").clicked() {
                            if !self.name.is_empty() {
                                if let Some(ref sender) = self.resp_sender {
                                    if is_create {
                                        let mut snippet = Snippet::new_snippet(
                                            self.name.clone(),
                                            self.content.clone(),
                                            self.selected_parent.clone(),
                                        );
                                        snippet.synced = false;
                                        let _ = sender.send(UiResponse::SnippetCreated(snippet));
                                    } else if let Some(s) = edit_snippet {
                                        let mut updated = s;
                                        updated.name = self.name.clone();
                                        updated.content = Some(self.content.clone());
                                        updated.synced = false;
                                        let _ = sender.send(UiResponse::SnippetUpdated(updated));
                                    }
                                }
                            }
                            self.hide();
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.hide();
                        }
                    });
                }
            });
    }
}
