#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
#[cfg(windows)]
mod audio;
mod background_loops;
mod background_motion;
mod drag_out;
mod gallery;
mod game_icons;
mod i18n;
mod icons;
mod library;
mod log;
mod player;
mod process;
mod scrub;
mod settings;
mod steam_icons;
mod still_preview;
mod swirl;
mod theme;
mod widgets;

use app::CutGutApp;

pub(crate) fn app_icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/cutgut.png"))
        .expect("embedded CutGut icon must be a valid PNG")
}

fn main() -> eframe::Result<()> {
    // Frame pacing and `waveOut` both round their waits to the system timer
    // tick; at the Windows default (15.6 ms) that is a whole frame of judder.
    let _timer = player::TimerResolution::raise();
    log::startup();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 860.0])
            .with_min_inner_size([900.0, 620.0])
            .with_icon(app_icon())
            .with_title("CutGut"),
        ..Default::default()
    };

    eframe::run_native(
        "CutGut",
        options,
        Box::new(|cc| Ok(Box::new(CutGutApp::new(cc)))),
    )
}
