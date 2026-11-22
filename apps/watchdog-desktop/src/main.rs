use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use watchdog_ai::ask_local;
use watchdog_capture::{CaptureWorker, SharedCaptureError, SharedFrame};
use watchdog_core::WatchGate;
use watchdog_memory::MemoryStore;

type SharedAnswer = Arc<Mutex<Option<String>>>;

struct WatchdogApp {
    gate: WatchGate,
    latest_frame: SharedFrame,
    capture_error: SharedCaptureError,
    _capture_worker: CaptureWorker,
    preview_texture: Option<egui::TextureHandle>,
    question: String,
    answer: String,
    model: String,
    status: String,
    pending: Arc<AtomicBool>,
    completed_answer: SharedAnswer,
    memory: Option<MemoryStore>,
    memory_notes: Vec<String>,
    memory_input: String,
    memory_status: String,
}

impl WatchdogApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let gate = WatchGate::new_off();
        let latest_frame: SharedFrame = Arc::new(Mutex::new(None));
        let capture_error: SharedCaptureError = Arc::new(Mutex::new(None));
        let worker = CaptureWorker::start(
            gate.clone(),
            Arc::clone(&latest_frame),
            Arc::clone(&capture_error),
        );

        let (memory, memory_status) = open_memory();
        let memory_notes = memory
            .as_ref()
            .and_then(|store| store.list_observations().ok())
            .unwrap_or_default();

        Self {
            gate,
            latest_frame,
            capture_error,
            _capture_worker: worker,
            preview_texture: None,
            question: String::new(),
            answer: "Turn WATCH on to preview your screen, then ask a question about the interface.".into(),
            model: "qwen2.5vl:3b".into(),
            status: "WATCH is OFF. No screen capture is running.".into(),
            pending: Arc::new(AtomicBool::new(false)),
            completed_answer: Arc::new(Mutex::new(None)),
            memory,
            memory_notes,
            memory_input: String::new(),
            memory_status,
        }
    }

    fn set_watch(&mut self, enabled: bool) {
        if enabled {
            self.gate.enable();
            self.status = "WATCH is ON. The visible preview is active.".into();
        } else {
            self.gate.disable();
            if let Ok(mut frame) = self.latest_frame.lock() {
                *frame = None;
            }
            self.preview_texture = None;
            self.status = "WATCH is OFF. The capture gate is closed and preview cleared.".into();
        }
    }

    fn ask_local(&mut self) {
        let question = self.question.trim().to_owned();
        if question.is_empty() || self.pending.swap(true, Ordering::AcqRel) {
            return;
        }

        let frame = if self.gate.is_enabled() {
            self.latest_frame.lock().ok().and_then(|slot| slot.clone())
        } else {
            None
        };
        let model = self.model.trim().to_owned();
        let pending = Arc::clone(&self.pending);
        let completed = Arc::clone(&self.completed_answer);

        self.answer.clear();
        self.status = "Waiting for the local model…".into();

        thread::spawn(move || {
            let result = ask_local(
                "http://127.0.0.1:11434/api/chat",
                &model,
                &question,
                frame.as_ref(),
            )
            .unwrap_or_else(|error| format!("Local AI error: {error}"));

            if let Ok(mut slot) = completed.lock() {
                *slot = Some(result);
            }
            pending.store(false, Ordering::Release);
        });
    }

    fn save_memory_note(&mut self) {
        let note = self.memory_input.trim().to_owned();
        if note.is_empty() {
            self.memory_status = "Write a note before saving.".into();
            return;
        }

        match self.memory.as_ref() {
            Some(store) => match store.save_observation(&note) {
                Ok(()) => {
                    self.memory_input.clear();
                    self.memory_notes = store.list_observations().unwrap_or_default();
                    self.memory_status = "Saved locally. You can clear these notes below.".into();
                }
                Err(error) => self.memory_status = format!("Could not save note: {error}"),
            },
            None => self.memory_status = "Local memory is unavailable. Check the app data folder permissions.".into(),
        }
    }

    fn clear_memory(&mut self) {
        match self.memory.as_ref() {
            Some(store) => match store.clear() {
                Ok(()) => {
                    self.memory_notes.clear();
                    self.memory_status = "All saved notes were cleared.".into();
                }
                Err(error) => self.memory_status = format!("Could not clear memory: {error}"),
            },
            None => self.memory_status = "Local memory is unavailable.".into(),
        }
    }
}

impl Drop for WatchdogApp {
    fn drop(&mut self) {
        self.gate.disable();
        if let Ok(mut frame) = self.latest_frame.lock() {
            *frame = None;
        }
    }
}

impl eframe::App for WatchdogApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(mut answer) = self.completed_answer.lock() {
            if let Some(answer) = answer.take() {
                self.answer = answer;
                self.status = "Response received from local Ollama.".into();
            }
        }

        if let Ok(error) = self.capture_error.lock() {
            if let Some(error) = error.as_ref() {
                self.status = format!("Capture status: {error}");
            }
        }

        egui::TopBottomPanel::top("watchdog_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("WATCHDOG");
                ui.label(egui::RichText::new("LOCAL VISUAL ASSISTANT").small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.gate.is_enabled() { "■  STOP WATCH" } else { "▶  START WATCH" };
                    if ui.add_sized([150.0, 36.0], egui::Button::new(label)).clicked() {
                        self.set_watch(!self.gate.is_enabled());
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.gate.is_enabled() {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "● WATCH ON");
                } else {
                    ui.colored_label(egui::Color32::GRAY, "● WATCH OFF");
                }
                ui.label(&self.status);
            });
            ui.separator();

            ui.columns(2, |columns| {
                columns[0].heading("Screen preview");
                columns[0].label("Frames stay in memory only; they are not written to disk.");

                let current_frame = self.latest_frame.lock().ok().and_then(|slot| slot.clone());
                if self.gate.is_enabled() {
                    if let Some(frame) = current_frame {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [frame.width as usize, frame.height as usize],
                            &frame.rgba,
                        );
                        if let Some(texture) = self.preview_texture.as_mut() {
                            texture.set(image, egui::TextureOptions::LINEAR);
                        } else {
                            self.preview_texture = Some(columns[0].ctx().load_texture(
                                "watchdog-preview",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }

                        if let Some(texture) = self.preview_texture.as_ref() {
                            let available = columns[0].available_size();
                            let size = texture.size_vec2();
                            let scale = (available.x / size.x)
                                .min((available.y - 20.0).max(80.0) / size.y)
                                .min(1.0);
                            columns[0].image((texture.id(), size * scale));
                        }
                    } else {
                        columns[0].add_space(32.0);
                        columns[0].label("Waiting for the first frame…");
                    }
                } else {
                    columns[0].add_space(32.0);
                    columns[0].label("Preview hidden while WATCH is OFF.");
                }

                columns[1].heading("Ask local AI");
                columns[1].label("Questions and optional frames go to Ollama on this PC.");
                columns[1].horizontal(|ui| {
                    ui.label("Model");
                    ui.text_edit_singleline(&mut self.model);
                });
                columns[1].add(
                    egui::TextEdit::multiline(&mut self.question)
                        .hint_text("Example: Where is the shop menu?")
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                let waiting = self.pending.load(Ordering::Acquire);
                if columns[1]
                    .add_enabled(!waiting, egui::Button::new("Ask local AI"))
                    .clicked()
                {
                    self.ask_local();
                }

                columns[1].add_space(10.0);
                columns[1].heading("Answer");
                egui::ScrollArea::vertical()
                    .max_height(210.0)
                    .show(&mut columns[1], |ui| {
                        ui.label(&self.answer);
                    });
            });

            ui.separator();
            ui.heading("Your interface memory");
            ui.label("Save text notes only when you choose. Screenshots are never stored in this database.");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.memory_input);
                if ui.button("Save note").clicked() {
                    self.save_memory_note();
                }
                if ui.button("Clear memory").clicked() {
                    self.clear_memory();
                }
            });
            ui.label(egui::RichText::new(&self.memory_status).small().weak());

            egui::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for note in self.memory_notes.iter().take(12) {
                        ui.label(format!("• {note}"));
                    }
                    if self.memory_notes.is_empty() {
                        ui.label("No saved notes yet.");
                    }
                });

            ui.separator();
            ui.label(egui::RichText::new(
                "Privacy: WATCH starts OFF. STOP closes capture and clears the latest frame. No analytics, keylogging, credential collection, cloud uploads, or automatic clicks.",
            ).small().weak());
        });

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn open_memory() -> (Option<MemoryStore>, String) {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let directory = base.join("WATCHDOG");
    if let Err(error) = std::fs::create_dir_all(&directory) {
        return (None, format!("Local memory unavailable: {error}"));
    }

    match MemoryStore::open(directory.join("memory.sqlite3")) {
        Ok(store) => (Some(store), "SQLite memory is stored locally on this device.".into()),
        Err(error) => (None, format!("Could not open local memory: {error}")),
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1160.0, 820.0])
            .with_min_inner_size([860.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "WATCHDOG — Local Visual Assistant",
        options,
        Box::new(|cc| Ok(Box::new(WatchdogApp::new(cc)))),
    )
}
