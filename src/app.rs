use crate::games::{self, Game, Source};
use crate::launcher;
use egui::{Color32, RichText};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PRESETS: [u32; 9] = [5, 10, 15, 20, 30, 40, 50, 80, 100];
const LAUNCH_GAP: Duration = Duration::from_millis(40);

#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Preset(u32),
    More,
}

struct Running {
    game: Game,
    child: Child,
}

#[derive(Default)]
struct Shared {
    loaded: Option<(Vec<Game>, Source)>,
    running: Vec<Running>,
    launching: bool,
    target: usize,
    failed: usize,
    stop: bool,
    log: Vec<String>,
}

impl Shared {
    fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 300 {
            self.log.drain(..100);
        }
    }

    fn stop_all(&mut self) {
        self.stop = true;
        let n = self.running.len();
        for mut r in self.running.drain(..) {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
        if n > 0 {
            self.log(format!("Stopped {n} games."));
        }
    }
}

pub struct App {
    shared: Arc<Mutex<Shared>>,
    ordered: Vec<Game>,
    source: Option<Source>,
    loading: bool,
    choice: Choice,
    custom: u32,
    auto_stop: bool,
    auto_stop_min: u32,
    started: Option<Instant>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let app = Self {
            shared: Arc::default(),
            ordered: Vec::new(),
            source: None,
            loading: false,
            choice: Choice::Preset(10),
            custom: 150,
            auto_stop: false,
            auto_stop_min: 20,
            started: None,
        };
        app.load_list(&cc.egui_ctx);
        app
    }

    fn load_list(&self, ctx: &egui::Context) {
        let shared = self.shared.clone();
        let ctx = ctx.clone();
        shared.lock().unwrap().log("Loading game list…");
        std::thread::spawn(move || {
            let (list, source) = games::load();
            let mut s = shared.lock().unwrap();
            s.log(format!("{} games available ({}).", list.len(), source.label()));
            s.loaded = Some((list, source));
            ctx.request_repaint();
        });
    }

    fn count(&self) -> usize {
        let n = match self.choice {
            Choice::Preset(n) => n,
            Choice::More => self.custom,
        };
        (n as usize).min(self.ordered.len())
    }

    fn start(&mut self, ctx: &egui::Context) {
        let plan: Vec<Game> = self.ordered.iter().take(self.count()).cloned().collect();
        {
            let mut s = self.shared.lock().unwrap();
            s.stop = false;
            s.launching = true;
            s.target = plan.len();
            s.failed = 0;
            s.log(format!("Starting {} games…", plan.len()));
        }
        self.started = Some(Instant::now());

        let shared = self.shared.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for game in plan {
                if shared.lock().unwrap().stop {
                    break;
                }
                let result = launcher::start(&game);
                let mut s = shared.lock().unwrap();
                if s.stop {
                    // Stop was pressed while this one was starting.
                    if let Ok(mut child) = result {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    break;
                }
                match result {
                    Ok(child) => s.running.push(Running { game, child }),
                    Err(e) => {
                        s.failed += 1;
                        s.log(format!("Could not start {}: {e}", game.name));
                    }
                }
                drop(s);
                ctx.request_repaint();
                std::thread::sleep(LAUNCH_GAP);
            }
            let mut s = shared.lock().unwrap();
            s.launching = false;
            if !s.stop {
                let n = s.running.len();
                s.log(format!("{n} games running. Discord should pick them up within a minute."));
            }
            ctx.request_repaint();
        });
    }

    fn stop(&mut self) {
        self.shared.lock().unwrap().stop_all();
        self.started = None;
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Pick up a freshly loaded list and notice games that were closed.
        let (running, launching, target, failed) = {
            let mut s = self.shared.lock().unwrap();
            if let Some((list, source)) = s.loaded.take() {
                self.ordered = games::order(&list);
                self.source = Some(source);
                self.loading = false;
            }
            let before = s.running.len();
            s.running.retain_mut(|r| matches!(r.child.try_wait(), Ok(None)));
            let gone = before - s.running.len();
            if gone > 0 {
                s.log(format!("{gone} game(s) were closed outside the app."));
            }
            (s.running.len(), s.launching, s.target, s.failed)
        };
        if self.source.is_none() {
            self.loading = true;
        }
        let active = running > 0 || launching;
        if !active && self.started.is_some() && target > 0 && failed < target {
            // Everything was closed from outside.
            self.started = None;
        }
        if let (true, Some(t)) = (self.auto_stop, self.started) {
            if active && t.elapsed() >= Duration::from_secs(self.auto_stop_min as u64 * 60) {
                self.shared.lock().unwrap().log("Auto-stop time reached.");
                self.stop();
            }
        }
        if active {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Faking game activity may break Discord's Terms of Service. Use at your own risk.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Discord Game Variety Badge");
            ui.horizontal(|ui| {
                if self.loading {
                    ui.spinner();
                    ui.label("Loading Discord's game list…");
                } else if let Some(src) = self.source {
                    ui.label(format!("{} games available · list from {}", self.ordered.len(), src.label()));
                    if ui.add_enabled(!active, egui::Button::new("Refresh").small()).clicked() {
                        self.source = None;
                        self.load_list(ctx);
                    }
                }
            });
            ui.add_space(10.0);

            // How many games.
            ui.label(RichText::new("How many games should Discord see?").strong());
            ui.add_space(4.0);
            ui.add_enabled_ui(!active, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for n in PRESETS {
                        let label = RichText::new(n.to_string()).size(16.0);
                        let btn = egui::Button::new(label).selected(self.choice == Choice::Preset(n))
                            .min_size(egui::vec2(48.0, 32.0));
                        if ui.add(btn).clicked() {
                            self.choice = Choice::Preset(n);
                        }
                    }
                    let btn = egui::Button::new(RichText::new("100+").size(16.0)).selected(self.choice == Choice::More)
                        .min_size(egui::vec2(48.0, 32.0));
                    if ui.add(btn).clicked() {
                        self.choice = Choice::More;
                    }
                });
                if self.choice == Choice::More {
                    ui.horizontal(|ui| {
                        ui.label("Number of games:");
                        let max = (self.ordered.len() as u32).max(101);
                        ui.add(egui::DragValue::new(&mut self.custom).range(101..=max).speed(1.0));
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.auto_stop, "Stop automatically after");
                    ui.add_enabled(
                        self.auto_stop,
                        egui::DragValue::new(&mut self.auto_stop_min).range(1..=600).suffix(" min"),
                    );
                });
            });
            ui.add_space(10.0);

            // Start / stop.
            let ready = !self.ordered.is_empty();
            ui.horizontal(|ui| {
                if active {
                    let stop = egui::Button::new(RichText::new("Stop all").size(18.0).color(Color32::WHITE))
                        .fill(Color32::from_rgb(190, 50, 50))
                        .min_size(egui::vec2(160.0, 40.0));
                    if ui.add(stop).clicked() {
                        self.stop();
                    }
                } else {
                    let label = format!("Start {} games", self.count());
                    let start = egui::Button::new(RichText::new(label).size(18.0).color(Color32::WHITE))
                        .fill(Color32::from_rgb(88, 101, 242))
                        .min_size(egui::vec2(160.0, 40.0));
                    if ui.add_enabled(ready, start).clicked() {
                        self.start(ctx);
                    }
                    if ui.add_enabled(ready, egui::Button::new("Shuffle")).on_hover_text(
                        "Popular games stay on top; the rest are picked again at random.",
                    ).clicked() {
                        games::reshuffle(&mut self.ordered);
                    }
                }
            });

            if active {
                ui.add_space(6.0);
                let elapsed = self.started.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                let mut status = format!(
                    "Running {running}/{target} · {:02}:{:02}",
                    elapsed / 60,
                    elapsed % 60
                );
                if failed > 0 {
                    status.push_str(&format!(" · {failed} failed"));
                }
                ui.horizontal(|ui| {
                    if launching {
                        ui.spinner();
                    }
                    ui.label(RichText::new(status).strong());
                });
                if launching && target > 0 {
                    ui.add(egui::ProgressBar::new((running + failed) as f32 / target as f32));
                }
            }

            ui.add_space(10.0);
            ui.separator();

            // The games.
            let s = self.shared.lock().unwrap();
            let names: Vec<(String, bool)> = if active {
                s.running.iter().map(|r| (r.game.name.clone(), true)).collect()
            } else {
                self.ordered.iter().take(self.count()).map(|g| (g.name.clone(), false)).collect()
            };
            let log = s.log.clone();
            drop(s);

            ui.label(if active { "Running now:" } else { "Will start:" });
            egui::ScrollArea::vertical()
                .id_salt("games")
                .max_height((ui.available_height() - 130.0).max(120.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (i, (name, on)) in names.iter().enumerate() {
                        let dot = if *on { RichText::new("●").color(Color32::from_rgb(35, 165, 90)) } else { RichText::new("○").weak() };
                        ui.horizontal(|ui| {
                            ui.label(dot);
                            ui.label(format!("{:>3}. {name}", i + 1));
                        });
                    }
                });

            ui.add_space(6.0);
            egui::CollapsingHeader::new("Log").default_open(false).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("log")
                    .max_height(100.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &log {
                            ui.label(RichText::new(line).monospace().small());
                        }
                    });
                if ui
                    .add_enabled(!active, egui::Button::new("Delete games folder"))
                    .on_hover_text("Removes the dummy game files next to this app.")
                    .clicked()
                {
                    let msg = match launcher::clean() {
                        Ok(()) => "Deleted the games folder.".to_string(),
                        Err(e) => format!("Could not delete the games folder: {e}"),
                    };
                    self.shared.lock().unwrap().log(msg);
                }
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.lock().unwrap().stop_all();
    }
}
