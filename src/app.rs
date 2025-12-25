use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use ehttp::Request;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    api_key: String,

    #[serde(skip)]
    text: Option<String>,

    #[serde(skip)] // This how you opt-out of serialization of a field
    value: f32,
    #[serde(skip)]
    rx: Option<Receiver<Result<String, String>>>
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            api_key: "Insert api key".to_owned(),
            value: 2.7,
            text: None,
            rx: None
        }
    }
}

impl TemplateApp {
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
            headers: ehttp::Headers::new(&[
                ("Authorization", &format!("Bearer {}", api_key)),
            ]),
            ..Request::get(url)
        };
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            match result {
                Ok(res) => {
                    let _ = tx.send(Ok(res.text().unwrap().to_string()));
                }
                Err(res) => {
                    let _ = tx.send(Err(res.to_string()));
                }
            }
        });
    }

}

impl eframe::App for TemplateApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if request completed
        if self.text.is_none() {
            if let Some(rx) = &self.rx {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(body) => self.text = Some(body),
                        Err(e) => self.text = Some(e),
                    }
                    self.rx = None;
                }
            }
        }

        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui
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

            ui.separator();

            if ui.button("reload").clicked() {
                self.text = None;
                self.make_request("https://mycelia.nel.re/api/messages");
            }

            if let Some(text) = &self.text {
                ui.label(text);
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

