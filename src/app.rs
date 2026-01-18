use crate::app::mpsc::channel;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use ehttp::Request;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use egui::{FontId, ScrollArea, TextEdit};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct MyceliaState {
    api_key: String,
    entries: Vec<Entry>,

    #[serde(skip)]
    rx: Receiver<Result<Vec<Entry>, String>>,
    #[serde(skip)]
    tx: Sender<Result<Vec<Entry>, String>>,
}

impl Default for MyceliaState {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            api_key: String::new(),
            entries: Vec::new(),
            rx,
            tx,
        }
    }
}

impl MyceliaState {
    pub fn save_entry(&mut self, entry: &mut Entry) {

        if let Some(id) = &entry.id {
            // Update cache
            self.entries.iter_mut().find(|e| e.id == entry.id)
                .map(|e| e.text = entry.text.clone() );

            // Update backend
            println!("Saving entry with id {:?}", id);
            let url = format!("https://mycelia.nel.re/api/entry/{}", id);
            let api_key = self.api_key.clone();

            let request = Request {
                method: "PATCH".to_string(),
                url: url.to_string(),
                headers: ehttp::Headers::new(&[
                    ("Authorization", &format!("Bearer {}", api_key)),
                    ("Content-Type", "application/json")
                ]),
                mode: Default::default(),
                body: serde_json::to_string(&entry).unwrap().into_bytes(),
                timeout: None,
            };
            ehttp::fetch(
                request,
                move |result: ehttp::Result<ehttp::Response>| match result {
                    Ok(res) => {
                        if res.ok {
                            println!("OK");
                        }
                    }
                    Err(res) => {
                        println!("{}", res.to_string());
                    }
                },
            );

            return;
        }

        // Does the entry have an id yet?
        if entry.id.is_none() {
            // Save it and receive an id
            let url = format!("https://mycelia.nel.re/api/entry");
            let api_key = self.api_key.clone();

            let request = Request {
                method: "POST".to_string(),
                url: url.to_string(),
                headers: ehttp::Headers::new(&[
                    ("Authorization", &format!("Bearer {}", api_key)),
                    ("Content-Type", "application/json")
                ]),
                mode: Default::default(),
                body: serde_json::to_string(&entry).unwrap().into_bytes(),
                timeout: None,
            };

            let tx = self.tx.clone();
            ehttp::fetch(
                request,
                move |result: ehttp::Result<ehttp::Response>| match result {
                    Ok(res) => {
                        if res.ok {
                            match serde_json::from_str::<Entry>(&res.text().unwrap()) {
                                Ok(entry) => {
                                    let _ = tx.send(Ok(vec![entry]));
                                }
                                Err(e) => {
                                    println!("Body {:?}", res.text());
                                    println!("Failed to parse json: {}", e);
                                    let _ = tx.send(Err("Failed to parse json".to_string()));
                                }
                            }
                        } else {
                            println!("Request not okt; {:?}", res.text());
                        }
                    }
                    Err(res) => {
                        println!("{}", res.to_string());
                    }
                },
            );

        }

    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[derive(PartialEq)]
pub struct Entry {
    pub id: Option<String>,
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

    pub(crate) fn show(&mut self, ui: &mut egui::Ui, state: &mut MyceliaState) {

        if let Some(entry) = &mut self.entry {
            ui.horizontal(|ui| {
                if ui.button("view").clicked() {
                    self.state = EditorState::View;
                }
                if ui.button("edit").clicked() {
                    self.state = EditorState::Edit;
                }
                if ui.button("save").clicked() {
                    state.save_entry(entry);
                }
            });

            ScrollArea::vertical()
                .id_salt("yoo")
                .show(ui, |ui| {
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
                    match &entry.id {
                        None => {
                            ui.label("New entry");
                        }
                        Some(id) => {
                            ui.label(format!("ID: {}", id));
                        }
                    }
                    TextEdit::multiline(&mut entry.text)
                        .font(FontId::monospace(12.0))
                        .desired_rows(10)
                        .show(ui);
                }
            }
                });
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

    editor_component: EditorComponent,

    #[serde(skip)]
    view_entry: Option<Entry>,

    m_state: MyceliaState,
}

impl Default for MyceliaApp {
    fn default() -> Self {
        let (tx, rx) = channel();

        Self {
            first_frame: true,
            editor_component: Default::default(),
            view_entry: None,
            m_state: MyceliaState {
                entries: vec![],
                api_key: "Insert api key".to_owned(),
                rx,
                tx
            },
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

    fn load_messages(&mut self) {
        let url = "https://mycelia.nel.re/api/messages";
        let api_key = self.m_state.api_key.clone();
        let tx = self.m_state.tx.clone();

        let request = Request {
            headers: ehttp::Headers::new(&[("Authorization", &format!("Bearer {}", api_key))]),
            ..Request::get(url)
        };
        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
                Ok(res) => {
                    if res.ok  {
                        match serde_json::from_str::<Vec<Entry>>(&res.text().unwrap()) {
                            Ok(entries) => {
                                let _ = tx.send(Ok(entries));
                            }
                            Err(_) => {
                                let _ = tx.send(Err("Failed to parse json".to_string()));
                            }
                        }
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
            self.load_messages();
        }

        // Parse messages
        if let Ok(result) = self.m_state.rx.try_recv() {
            match result {
                Ok(body) => {

                    // If a new entry is returned while we're editing then we should update the view
                    if let Some(edit_entry) = &mut self.editor_component.entry {
                        let new_entry = body.first().unwrap().clone();
                        if edit_entry.id.is_none() && edit_entry.text == new_entry.text {
                            *edit_entry = new_entry;
                        }
                    }

                    // Update new entries
                    for entry in body {

                        let cache_entry = self.m_state.entries.iter_mut().find(|e| {
                            e.id == entry.id
                        });

                        if let Some(cache_entry) = cache_entry {
                            *cache_entry = entry.clone();
                        } else {
                            self.m_state.entries.push(entry);
                        }
                    }
                }
                Err(e) => {
                    print!("{}", e);
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
                ui.text_edit_singleline(&mut self.m_state.api_key);
            });

            if ui.button("reload").clicked() {
                self.load_messages();
            }

            if ui.button("new").clicked() {
                self.editor_component.focus( Entry {
                    id: None,
                    text: "".to_string(),
                });
            }

            ui.separator();

            ui.columns(2, |ui| {
                egui::ScrollArea::vertical().show(&mut ui[0], |ui| {
                    if self.m_state.entries.is_empty() {
                        ui.label("Loading...");
                    }
                    egui::Grid::new("entries")
                        .num_columns(2)
                        .max_col_width(ui.available_width()) // Why is this needed?
                        .striped(true)
                        .show(ui, |ui| {
                            for entry in self.m_state.entries.iter().rev() {
                                if ui.button("open").clicked() {
                                    self.editor_component.focus(entry.clone());
                                }

                                ui.label(&entry.text);
                                ui.end_row();
                            }
                        });
                });

                self.editor_component.show(&mut ui[1], &mut self.m_state);
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}
