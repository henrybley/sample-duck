use crate::app::SampleDuckApp;

mod app;
mod audio_player;
mod db;
mod sample;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Sample Duck",
        options,
        Box::new(|_cc| Ok(Box::new(SampleDuckApp::new()))),
    )
}
