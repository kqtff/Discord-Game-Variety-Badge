// No console window on Windows, for both the GUI and the dummy game processes.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod games;
mod launcher;
mod runner;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // The same .exe doubles as the dummy "game": copies of it are launched
    // under each game's executable name with this flag, and just stay alive.
    if args.iter().any(|a| a == runner::FLAG) {
        runner::run(&args);
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Discord Game Variety Badge")
            .with_inner_size([580.0, 680.0])
            .with_min_inner_size([440.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Discord Game Variety Badge",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
