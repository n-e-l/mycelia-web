use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use ehttp::Request;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Entry {
    pub id: String,
    pub text: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
enum EditorState {
    View,
    Edit
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
struct EditorComponent {
    entry: Option<Entry>,
    state: EditorState
}

impl EditorComponent {

    pub fn focus(&mut self, entry: Entry) {
        self.entry = Some(entry);
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {

        if let Some(entry) = &mut self.entry {
            ui.horizontal(|ui| {
                if ui.button("view").clicked() {
                    self.state = EditorState::View;
                }
                if ui.button("edit").clicked() {
                    self.state = EditorState::Edit;
                }
            });

            match self.state {
                EditorState::View => {
                    let mut cache = CommonMarkCache::default();
                    CommonMarkViewer::new().show(
                        ui,
                        &mut cache,
                        &mut entry.text.as_str(),
                    );
                }
                EditorState::Edit => {
                    ui.text_edit_multiline(&mut entry.text);
                }
            }
        } else {
            ui.label("Nothing selected");
        }
        return;
    }
}

impl Default for EditorComponent {
    fn default() -> Self {
        EditorComponent { entry: None, state: EditorState::View }
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MyceliaApp {
    #[serde(skip)]
    first_frame: bool,

    api_key: String,

    editor_component: EditorComponent,

    #[serde(skip)]
    view_entry: Option<Entry>,

    #[serde(skip)]
    text: Option<Result<String, String>>,
    #[serde(skip)]
    entries: Vec<Entry>,

    #[serde(skip)]
    rx: Option<Receiver<Result<String, String>>>,
}

impl Default for MyceliaApp {
    fn default() -> Self {
        Self {
            first_frame: true,
            api_key: "Insert api key".to_owned(),
            editor_component: Default::default(),
            text: None,
            view_entry: None,
            entries: vec![],
            rx: None,
        }
    }
}

impl MyceliaApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    fn make_request(&mut self, url: &str) {
        let url = url.to_string();
        let api_key = self.api_key.clone();
        let (tx, rx) = mpsc::channel();

        self.rx = Some(rx);

        let request = Request {
            headers: ehttp::Headers::new(&[("Authorization", &format!("Bearer {}", api_key))]),
            ..Request::get(url)
        };
        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
                Ok(res) => {
                    if res.ok {
                        let _ = tx.send(Ok(res.text().unwrap().to_string()));
                    } else {
                        let _ = tx.send(Err(res.text().unwrap().to_string()));
                    }
                }
                Err(res) => {
                    let _ = tx.send(Err(res.to_string()));
                }
            },
        );
    }
}

impl eframe::App for MyceliaApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            self.make_request("https://mycelia.nel.re/api/messages");
        }

        // Check if request completed
        if self.text.is_none() {
            if let Some(rx) = &self.rx {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(body) => {
                            self.entries.clear();
                            match serde_json::from_str::<Vec<Entry>>(&body) {
                                Ok(entries) => {
                                    self.entries = entries;
                                    self.text = Some(Ok("".to_string()));
                                }
                                Err(e) => {
                                    self.text = Some(Err(format!("Failed to parse JSON: {}", e)));
                                }
                            }
                        }
                        Err(e) => self.text = Some(Err(e)),
                    }
                    self.rx = None;
                }
            }
        }

        ctx.set_visuals(egui::Visuals::dark());

        // There is nothing in the top bar for web (yet)
        let is_web = cfg!(target_arch = "wasm32");
        if !is_web {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Mycelia");

            ui.horizontal(|ui| {
                ui.label("API key: ");
                ui.text_edit_singleline(&mut self.api_key);
            });

            if ui.button("reload").clicked() {
                self.text = None;
                self.make_request("https://mycelia.nel.re/api/messages");
            }

            ui.separator();

            ui.columns(2, |ui| {
                egui::ScrollArea::vertical().show(&mut ui[0], |ui| {
                    if self.entries.is_empty() {
                        ui.label("Loading...");
                    }
                    egui::Grid::new("entries")
                        .num_columns(2)
                        .max_col_width(ui.available_width()) // Why is this needed?
                        .striped(true)
                        .show(ui, |ui| {
                            for entry in self.entries.iter().rev() {
                                if ui.button("open").clicked() {
                                    self.editor_component.focus(entry.clone());
                                }

                                ui.label(&entry.text);
                                ui.end_row();
                            }
                        });
                });

                self.editor_component.show(&mut ui[1]);
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}
