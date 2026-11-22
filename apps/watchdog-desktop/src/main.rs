use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use watchdog_ai::{ask_local, check_local_server, coach_game, GameCoachUpdate};
use watchdog_capture::{CaptureWorker, SharedCaptureError, SharedFrame};
use watchdog_core::WatchGate;
use watchdog_memory::MemoryStore;

type SharedAnswer = Arc<Mutex<Option<String>>>;
type SharedCoachResult = Arc<Mutex<Option<Result<GameCoachUpdate, String>>>>;
type SharedServerResult = Arc<Mutex<Option<Result<Vec<String>, String>>>>;

#[derive(Default)]
struct HudState {
    visible: bool,
    game_name: String,
    status: String,
    current_goal: String,
    scene_summary: String,
    next_step: String,
    question: String,
    confidence: String,
    answer_input: String,
    pending_answer: Option<String>,
    analyze_now: bool,
    save_lesson: bool,
    stop_session: bool,
}

struct WatchdogApp {
    gate: WatchGate,
    latest_frame: SharedFrame,
    capture_error: SharedCaptureError,
    _capture_worker: CaptureWorker,
    preview_texture: Option<egui::TextureHandle>,
    question: String,
    answer: String,
    model: String,
    ollama_server: String,
    status: String,
    pending: Arc<AtomicBool>,
    completed_answer: SharedAnswer,
    memory: Option<MemoryStore>,
    memory_notes: Vec<String>,
    memory_input: String,
    memory_status: String,

    game_name: String,
    coach_session_active: bool,
    auto_guidance: bool,
    coach_waiting_for_answer: bool,
    next_auto_analysis: Instant,
    session_context: Vec<String>,
    last_coach_update: Option<GameCoachUpdate>,
    coach_hud: Arc<Mutex<HudState>>,
    coach_pending: Arc<AtomicBool>,
    coach_result: SharedCoachResult,

    server_check_pending: Arc<AtomicBool>,
    server_result: SharedServerResult,
    server_status: String,
    auto_check_started: bool,
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
            ollama_server: "http://127.0.0.1:11434".into(),
            status: "WATCH is OFF. No screen capture is running.".into(),
            pending: Arc::new(AtomicBool::new(false)),
            completed_answer: Arc::new(Mutex::new(None)),
            memory,
            memory_notes,
            memory_input: String::new(),
            memory_status,

            game_name: "Roblox".into(),
            coach_session_active: false,
            auto_guidance: true,
            coach_waiting_for_answer: false,
            next_auto_analysis: Instant::now(),
            session_context: Vec::new(),
            last_coach_update: None,
            coach_hud: Arc::new(Mutex::new(HudState::default())),
            coach_pending: Arc::new(AtomicBool::new(false)),
            coach_result: Arc::new(Mutex::new(None)),

            server_check_pending: Arc::new(AtomicBool::new(false)),
            server_result: Arc::new(Mutex::new(None)),
            server_status: "Searching for local Ollama automatically…".into(),
            auto_check_started: false,
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

    fn start_game_guide(&mut self) {
        if self.game_name.trim().is_empty() {
            self.game_name = "Roblox".into();
        }
        self.set_watch(true);
        self.coach_session_active = true;
        self.auto_guidance = true;
        self.coach_waiting_for_answer = false;
        self.session_context.clear();
        self.last_coach_update = None;
        self.next_auto_analysis = Instant::now() + Duration::from_secs(3);

        if let Ok(mut hud) = self.coach_hud.lock() {
            *hud = HudState {
                visible: true,
                game_name: self.game_name.trim().to_owned(),
                status: "Joining the session… waiting for the first game frame.".into(),
                next_step: "Open your game. WATCHDOG will inspect the visible game screen.".into(),
                confidence: "low".into(),
                ..Default::default()
            };
        }
        self.status = format!("{} game-learning session started. The local AI will inspect the screen.", self.game_name);
    }

    fn stop_game_guide(&mut self) {
        self.coach_session_active = false;
        self.auto_guidance = false;
        self.coach_waiting_for_answer = false;
        self.set_watch(false);
        if let Ok(mut hud) = self.coach_hud.lock() {
            hud.visible = false;
            hud.status = "Game guide stopped. Screen capture is OFF.".into();
        }
        self.status = "Game guide stopped. WATCH is OFF.".into();
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
        let endpoint = format!("{}/api/chat", self.ollama_server.trim().trim_end_matches('/'));
        let pending = Arc::clone(&self.pending);
        let completed = Arc::clone(&self.completed_answer);

        self.answer.clear();
        self.status = "Waiting for the local model…".into();

        thread::spawn(move || {
            let result = ask_local(&endpoint, &model, &question, frame.as_ref())
                .unwrap_or_else(|error| format!("Local AI error: {error}"));

            if let Ok(mut slot) = completed.lock() {
                *slot = Some(result);
            }
            pending.store(false, Ordering::Release);
        });
    }

    fn request_game_analysis(&mut self) {
        if !self.coach_session_active || self.coach_pending.swap(true, Ordering::AcqRel) {
            return;
        }

        let frame = self.latest_frame.lock().ok().and_then(|slot| slot.clone());
        let Some(frame) = frame else {
            self.coach_pending.store(false, Ordering::Release);
            if let Ok(mut hud) = self.coach_hud.lock() {
                hud.status = "Waiting for the game frame. Keep WATCH on and try again in a moment.".into();
            }
            self.next_auto_analysis = Instant::now() + Duration::from_secs(3);
            return;
        };

        let model = self.model.trim().to_owned();
        let endpoint = self.ollama_server.trim().to_owned();
        let game_name = self.game_name.trim().to_owned();
        let player_context = self.session_context.clone();
        let prefix = format!("[Game: {}]", game_name.to_lowercase());
        let approved_notes = self
            .memory
            .as_ref()
            .and_then(|store| store.list_observations().ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|note| note.to_lowercase().starts_with(&prefix))
            .collect::<Vec<_>>();

        let pending = Arc::clone(&self.coach_pending);
        let completed = Arc::clone(&self.coach_result);
        self.next_auto_analysis = Instant::now() + Duration::from_secs(25);

        if let Ok(mut hud) = self.coach_hud.lock() {
            hud.status = "Local AI is inspecting the current game screen…".into();
        }

        thread::spawn(move || {
            let result = coach_game(
                &endpoint,
                &model,
                &game_name,
                &frame,
                &player_context,
                &approved_notes,
            );
            if let Ok(mut slot) = completed.lock() {
                *slot = Some(result);
            }
            pending.store(false, Ordering::Release);
        });
    }

    fn poll_background_tasks(&mut self) {
        if let Ok(mut completed) = self.completed_answer.lock() {
            if let Some(answer) = completed.take() {
                self.answer = answer;
                self.status = "Response received from local Ollama.".into();
            }
        }

        if let Ok(mut completed) = self.coach_result.lock() {
            if let Some(result) = completed.take() {
                match result {
                    Ok(update) => {
                        self.coach_waiting_for_answer = !update.question.trim().is_empty();
                        if let Ok(mut hud) = self.coach_hud.lock() {
                            hud.game_name = update.game_name.clone();
                            hud.current_goal = update.current_goal.clone();
                            hud.scene_summary = update.scene_summary.clone();
                            hud.next_step = update.next_step.clone();
                            hud.question = update.question.clone();
                            hud.confidence = update.confidence.clone();
                            hud.status = if self.coach_waiting_for_answer {
                                "I need one detail from you before I continue.".into()
                            } else {
                                "Screen inspected. I will check again during the active session.".into()
                            };
                        }
                        self.status = "Game coach updated from the current screen.".into();
                        self.last_coach_update = Some(update);
                        self.next_auto_analysis = Instant::now() + Duration::from_secs(25);
                    }
                    Err(error) => {
                        self.coach_waiting_for_answer = true;
                        if let Ok(mut hud) = self.coach_hud.lock() {
                            hud.status = format!("Local AI issue: {error}");
                            hud.question = "Can you check that Ollama is running and the selected vision model is installed?".into();
                        }
                        self.status = format!("Game coach could not analyze the screen: {error}");
                    }
                }
            }
        }

        if let Ok(mut completed) = self.server_result.lock() {
            if let Some(result) = completed.take() {
                self.server_status = match result {
                    Ok(models) if models.is_empty() => {
                        "Ollama is reachable, but it reports no downloaded models.".into()
                    }
                    Ok(models) if !models.is_empty() => {
                        let preferred = models.iter().find(|name| name.to_lowercase().contains("qwen2.5vl:3b"))
                            .or_else(|| models.iter().find(|name| {
                                let lower = name.to_lowercase();
                                lower.contains("vision") || lower.contains("vl") || lower.contains("llava")
                            }))
                            .or_else(|| models.first());
                        if let Some(model) = preferred {
                            self.model = model.clone();
                            format!("Connected automatically. Selected model: {model}. Installed models: {}", models.join(", "))
                        } else {
                            "Connected to Ollama, but no models were found.".into()
                        }
                    },
                    Err(error) => error,
                };
            }
        }
    }

    fn check_local_server(&mut self) {
        if self.server_check_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        self.server_status = "Automatically checking local Ollama…".into();
        let endpoint = self.ollama_server.trim().to_owned();
        let pending = Arc::clone(&self.server_check_pending);
        let completed = Arc::clone(&self.server_result);
        thread::spawn(move || {
            let result = check_local_server(&endpoint);
            if let Ok(mut slot) = completed.lock() {
                *slot = Some(result);
            }
            pending.store(false, Ordering::Release);
        });
    }

    fn save_game_lesson(&mut self) {
        let Some(update) = self.last_coach_update.clone() else {
            self.memory_status = "There is no coach finding to save yet.".into();
            return;
        };

        let player_detail = self.session_context.last().cloned().unwrap_or_default();
        let note = format!(
            "[Game: {}] Goal: {} | Screen: {} | Useful fact: {} | Suggested step: {} | Player explanation: {}",
            self.game_name.trim(),
            update.current_goal,
            update.scene_summary,
            update.learning,
            update.next_step,
            player_detail
        );

        match self.memory.as_ref() {
            Some(store) => match store.save_observation(&note) {
                Ok(()) => {
                    self.memory_notes = store.list_observations().unwrap_or_default();
                    self.memory_status = "Saved this game lesson locally. It can guide future sessions.".into();
                    if let Ok(mut hud) = self.coach_hud.lock() {
                        hud.status = "Lesson saved to your local game memory.".into();
                    }
                }
                Err(error) => self.memory_status = format!("Could not save game lesson: {error}"),
            },
            None => self.memory_status = "Local memory is unavailable.".into(),
        }
    }

    fn handle_hud_actions(&mut self) {
        let (answer, analyze, save, stop) = if let Ok(mut hud) = self.coach_hud.lock() {
            (
                hud.pending_answer.take(),
                std::mem::take(&mut hud.analyze_now),
                std::mem::take(&mut hud.save_lesson),
                std::mem::take(&mut hud.stop_session),
            )
        } else {
            (None, false, false, false)
        };

        let mut should_analyze = analyze;
        if let Some(answer) = answer.filter(|answer| !answer.trim().is_empty()) {
            self.session_context.push(format!("Player explanation: {}", answer.trim()));
            self.coach_waiting_for_answer = false;
            if let Ok(mut hud) = self.coach_hud.lock() {
                hud.question.clear();
                hud.status = "Thanks. I will use your explanation to understand the game.".into();
            }
            should_analyze = true;
        }

        if save {
            self.save_game_lesson();
        }
        if stop {
            self.stop_game_guide();
            return;
        }
        if should_analyze {
            self.request_game_analysis();
        }
    }

    fn show_game_hud(&mut self, ctx: &egui::Context) {
        if !self.coach_session_active {
            return;
        }
        let visible = self
            .coach_hud
            .lock()
            .map(|hud| hud.visible)
            .unwrap_or(false);
        if !visible {
            return;
        }

        let hud_state = Arc::clone(&self.coach_hud);
        let builder = egui::ViewportBuilder::default()
            .with_title("WATCHDOG Game Guide")
            .with_inner_size([370.0, 300.0])
            .with_min_inner_size([320.0, 230.0])
            .with_position(egui::pos2(16.0, 54.0))
            .with_always_on_top()
            .with_decorations(false)
            .with_resizable(true);

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("watchdog-game-guide-hud"),
            builder,
            move |overlay_ctx, _class| {
                egui::CentralPanel::default().show(overlay_ctx, |ui| {
                    if let Ok(mut hud) = hud_state.lock() {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("WATCHDOG").strong());
                            ui.label(egui::RichText::new(format!("{} GUIDE", hud.game_name.to_uppercase())).weak());
                            if ui.button("Hide").clicked() {
                                hud.visible = false;
                            }
                        });
                        ui.separator();

                        ui.label(egui::RichText::new("NEXT STEP").small().weak());
                        ui.label(egui::RichText::new(if hud.next_step.is_empty() {
                            "Inspecting the game screen…"
                        } else {
                            hud.next_step.as_str()
                        }).strong());

                        if !hud.current_goal.is_empty() {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("CURRENT GOAL").small().weak());
                            ui.label(&hud.current_goal);
                        }
                        if !hud.scene_summary.is_empty() {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(&hud.scene_summary).small());
                        }
                        if !hud.question.is_empty() {
                            ui.separator();
                            ui.label(egui::RichText::new("HELP ME UNDERSTAND").small().strong());
                            ui.label(&hud.question);
                            ui.add(
                                egui::TextEdit::singleline(&mut hud.answer_input)
                                    .hint_text("Type what this button or item does…")
                                    .desired_width(f32::INFINITY),
                            );
                            if ui.button("Send answer").clicked() {
                                let answer = hud.answer_input.trim().to_owned();
                                if !answer.is_empty() {
                                    hud.pending_answer = Some(answer);
                                    hud.answer_input.clear();
                                }
                            }
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Analyze now").clicked() {
                                hud.analyze_now = true;
                            }
                            if ui.button("Save lesson").clicked() {
                                hud.save_lesson = true;
                            }
                            if ui.button("Stop guide").clicked() {
                                hud.stop_session = true;
                            }
                        });
                        ui.label(egui::RichText::new(format!(
                            "{} · confidence: {}",
                            hud.status,
                            if hud.confidence.is_empty() { "—" } else { hud.confidence.as_str() }
                        )).small().weak());
                    }
                });
            },
        );
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
        self.poll_background_tasks();
        if !self.auto_check_started {
            self.auto_check_started = true;
            self.check_local_server();
        }
        self.handle_hud_actions();

        if let Ok(error) = self.capture_error.lock() {
            if let Some(error) = error.as_ref() {
                self.status = format!("Capture status: {error}");
            }
        }

        if self.coach_session_active
            && self.auto_guidance
            && !self.coach_waiting_for_answer
            && !self.coach_pending.load(Ordering::Acquire)
            && Instant::now() >= self.next_auto_analysis
        {
            self.request_game_analysis();
        }

        egui::TopBottomPanel::top("watchdog_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("WATCHDOG");
                ui.label(egui::RichText::new("LOCAL GAME COACH").small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.gate.is_enabled() { "■  STOP WATCH" } else { "▶  START WATCH" };
                    if ui.add_sized([150.0, 36.0], egui::Button::new(label)).clicked() {
                        self.set_watch(!self.gate.is_enabled());
                    }
                });
            });
        });

        let mut start_guide_clicked = false;
        let mut stop_guide_clicked = false;
        let mut analyze_clicked = false;
        let mut save_lesson_clicked = false;
        let mut ask_clicked = false;
        let mut server_check_clicked = false;
        let mut show_hud_clicked = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.gate.is_enabled() {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "● WATCH ON");
                } else {
                    ui.colored_label(egui::Color32::GRAY, "● WATCH OFF");
                }
                ui.label(&self.status);
            });

            egui::CollapsingHeader::new("Roblox and game learning session")
                .default_open(true)
                .show(ui, |ui| {
                    ui.label("Start this when you enter a game. WATCHDOG will inspect the visible game UI and guide you one step at a time.");
                    ui.horizontal(|ui| {
                        ui.label("Game");
                        ui.text_edit_singleline(&mut self.game_name);
                        ui.checkbox(&mut self.auto_guidance, "Automatic suggestions");
                    });
                    ui.horizontal(|ui| {
                        if !self.coach_session_active {
                            if ui.button("Start game guide").clicked() {
                                start_guide_clicked = true;
                            }
                        } else {
                            if ui.button("Analyze screen now").clicked() {
                                analyze_clicked = true;
                            }
                            if ui.button("Save current lesson").clicked() {
                                save_lesson_clicked = true;
                            }
                            if ui.button("Stop game guide").clicked() {
                                stop_guide_clicked = true;
                            }
                            let visible = self.coach_hud.lock().map(|hud| hud.visible).unwrap_or(false);
                            if !visible && ui.button("Show top-left HUD").clicked() {
                                show_hud_clicked = true;
                            }
                        }
                    });
                    ui.label(egui::RichText::new(
                        "The floating guide is an always-on-top window. It suggests actions but never clicks or plays for you."
                    ).small().weak());
                });

            ui.separator();
            ui.collapsing("Local AI server", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ollama address");
                    ui.text_edit_singleline(&mut self.ollama_server);
                });
                ui.horizontal(|ui| {
                    ui.label("Vision model");
                    ui.text_edit_singleline(&mut self.model);
                    if ui.add_enabled(
                        !self.server_check_pending.load(Ordering::Acquire),
                        egui::Button::new("Test server")
                    ).clicked() {
                        server_check_clicked = true;
                    }
                });
                ui.label(egui::RichText::new(&self.server_status).small().weak());
                ui.label(egui::RichText::new(
                    "Local-only endpoint. Default: http://127.0.0.1:11434. No cloud AI endpoint is used."
                ).small().weak());
            });

            ui.separator();
            ui.columns(2, |columns| {
                columns[0].heading("Screen preview");
                columns[0].label("Live frames stay in memory. WATCH does not save a video or screenshot file.");

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
                columns[1].label("Ask a question about the current screen.");
                columns[1].add(
                    egui::TextEdit::multiline(&mut self.question)
                        .hint_text("What does this menu do?")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if columns[1]
                    .add_enabled(!self.pending.load(Ordering::Acquire), egui::Button::new("Ask local AI"))
                    .clicked()
                {
                    ask_clicked = true;
                }
                columns[1].add_space(8.0);
                columns[1].heading("Answer");
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(&mut columns[1], |ui| ui.label(&self.answer));
            });

            ui.separator();
            ui.heading("Your local interface memory");
            ui.label("Save only text notes that you approve. Game lessons are kept separately by game name.");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.memory_input);
                if ui.button("Save note").clicked() {
                    let note = self.memory_input.trim().to_owned();
                    if !note.is_empty() {
                        if let Some(store) = self.memory.as_ref() {
                            match store.save_observation(&note) {
                                Ok(()) => {
                                    self.memory_input.clear();
                                    self.memory_notes = store.list_observations().unwrap_or_default();
                                    self.memory_status = "Saved locally.".into();
                                }
                                Err(error) => self.memory_status = format!("Could not save note: {error}"),
                            }
                        }
                    }
                }
                if ui.button("Clear memory").clicked() {
                    if let Some(store) = self.memory.as_ref() {
                        match store.clear() {
                            Ok(()) => {
                                self.memory_notes.clear();
                                self.memory_status = "All saved notes were cleared.".into();
                            }
                            Err(error) => self.memory_status = format!("Could not clear memory: {error}"),
                        }
                    }
                }
            });
            ui.label(egui::RichText::new(&self.memory_status).small().weak());
            egui::ScrollArea::vertical().max_height(90.0).show(ui, |ui| {
                for note in self.memory_notes.iter().take(8) {
                    ui.label(format!("• {note}"));
                }
                if self.memory_notes.is_empty() {
                    ui.label("No saved notes yet.");
                }
            });

            ui.separator();
            ui.label(egui::RichText::new(
                "Privacy: WATCH starts OFF. STOP clears the current frame. The local model sees only frames sent during your active session. No keylogging, credential collection, cloud uploads, or automatic game actions."
            ).small().weak());
        });

        if start_guide_clicked {
            self.start_game_guide();
        }
        if stop_guide_clicked {
            self.stop_game_guide();
        }
        if analyze_clicked {
            self.request_game_analysis();
        }
        if save_lesson_clicked {
            self.save_game_lesson();
        }
        if ask_clicked {
            self.ask_local();
        }
        if server_check_clicked {
            self.check_local_server();
        }
        if show_hud_clicked {
            if let Ok(mut hud) = self.coach_hud.lock() {
                hud.visible = true;
            }
        }

        self.show_game_hud(ctx);
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
            .with_inner_size([1180.0, 900.0])
            .with_min_inner_size([900.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "WATCHDOG — Local Visual Game Coach",
        options,
        Box::new(|cc| Ok(Box::new(WatchdogApp::new(cc)))),
    )
}
