use crate::audio_file::format_bytes;
use crate::{audio_player::AudioPlayer, db::DB, sample::Sample};
use std::rc::Rc;
use std::{fs, path::Path};

use slint::*;

mod audio_file;
mod audio_player;
mod db;
mod sample;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = SampleDuckApp::new();
    app.run()
}

pub struct SampleDuckApp {
    pub db: DB,
    pub audio_player: AudioPlayer,
    pub samples: Vec<Sample>,
    pub selected_sample: Option<Sample>,
    pub selected_sample_idx: Option<usize>,
    pub sample_notes_modal_open: bool,
}

impl SampleDuckApp {
    pub fn new() -> Self {
        let db = DB::new();

        let audio_player = AudioPlayer::new().unwrap();

        let samples = db.get_samples().unwrap();

        Self {
            db,
            audio_player,
            samples,
            selected_sample: None,
            selected_sample_idx: None,
            sample_notes_modal_open: false,
        }
    }

    pub fn run(mut self) -> Result<(), slint::PlatformError> {
        let ui = AppWindow::new()?;
        let app_weak = ui.as_weak();
        
        // Initial setup
        self.update_ui(&ui);
        
        // Move self into RefCell
        let app_ref = Rc::new(std::cell::RefCell::new(self));
        
        // Import button callback
        let app_clone = app_ref.clone();
        let app_weak_clone = app_weak.clone();
        ui.on_import_dir(move || {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                let mut app = app_clone.borrow_mut();
                app.import_samples_from_dir(&dir);
                app.query_samples();
                if let Some(ui) = app_weak_clone.upgrade() {
                    app.update_ui(&ui);
                }
            }
        });

        // Sample selection callback
        let app_clone = app_ref.clone();
        let app_weak_clone = app_weak.clone();
        ui.on_select_sample(move |index| {
            let mut app = app_clone.borrow_mut();
            app.select_sample(index as usize);
            if let Some(ui) = app_weak_clone.upgrade() {
                app.update_ui(&ui);
            }
        });

        // Rating update callback
        let app_clone = app_ref.clone();
        let app_weak_clone = app_weak.clone();
        ui.on_update_rating(move |sample_id, rating| {
            let mut app = app_clone.borrow_mut();
            app.db.update_rating(sample_id as isize, rating as u8);
            app.query_samples();
            if let Some(ui) = app_weak_clone.upgrade() {
                app.update_ui(&ui);
            }
        });

        // Play/pause toggle
        let app_clone = app_ref.clone();
        ui.on_toggle_play(move || {
            let mut app = app_clone.borrow_mut();
            app.audio_player.toggle_play_state();
        });


        let app_clone = app_ref.clone();
        ui.on_stop_play(move || {
            let mut app = app_clone.borrow_mut();
            app.audio_player.stop();
        });

        // Waveform click callback
        let app_clone = app_ref.clone();
        ui.on_waveform_clicked(move |position| {
            let mut app = app_clone.borrow_mut();
            app.audio_player.seek_to_position_percentage(position);
        });

        // Keyboard handling
        let app_clone = app_ref.clone();
        let app_weak_clone = app_weak.clone();
        ui.on_next_sample(move || {
            let mut app = app_clone.borrow_mut();
            app.select_next_sample();
            if let Some(ui) = app_weak_clone.upgrade() {
                app.update_ui(&ui);
            }
        });

        let app_clone = app_ref.clone();
        let app_weak_clone = app_weak.clone();
        ui.on_prev_sample(move || {
            let mut app = app_clone.borrow_mut();
            app.select_prev_sample();
            if let Some(ui) = app_weak_clone.upgrade() {
                app.update_ui(&ui);
            }
        });

        // Animation timer for waveform updates
        let app_clone = app_ref.clone();
        let app_weak_clone = app_weak.clone();
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, std::time::Duration::from_millis(16), move || {
            if let Some(ui) = app_weak_clone.upgrade() {
                let app = app_clone.borrow();
                if app.selected_sample.is_some() {
                    ui.set_playhead_position(app.audio_player.get_position_percentage());
                    // Update waveform data if needed
                    app.update_waveform(&ui);
                }
            }
        });

        ui.run()
    }

    fn update_ui(&self, ui: &AppWindow) {
        // Convert samples to Slint model
        let sample_items: Vec<SampleItem> = self.samples
            .iter()
            .map(|s| s.clone().into())
            .collect();
        
        let model = Rc::new(VecModel::from(sample_items));
        ui.set_samples(ModelRc::from(model));

        // Update selected sample details
        if let Some(selected) = &self.selected_sample {
            ui.set_selected_sample(selected.clone().into());
            ui.set_has_selected_sample(true);
            
            if let Some(idx) = self.selected_sample_idx {
                ui.set_selected_index(idx as i32);
            }
        } else {
            ui.set_has_selected_sample(false);
        }
    }

    fn update_waveform(&self, ui: &AppWindow) {
        if self.selected_sample.is_some() {
            let waveform_data: Vec<WaveformPoint> = self.audio_player
                .peak_samples
                .iter()
                .enumerate()
                .map(|(i, &(min, max))| {
                    let x = i as f32 / self.audio_player.peak_samples.len() as f32;
                    let position_percentage = self.audio_player.get_position_percentage();
                    let is_played = x <= position_percentage;
                    
                    WaveformPoint {
                        x: x,
                        min: min,
                        max: max,
                        is_played: is_played,
                    }
                })
                .collect();
            
            let model = Rc::new(VecModel::from(waveform_data));
            ui.set_waveform_data(ModelRc::from(model));
        }
    }

    fn select_sample(&mut self, sample_idx: usize) {
        if self.samples.len() > sample_idx {
            self.selected_sample_idx = Some(sample_idx);
            self.selected_sample = Some(self.samples[sample_idx].clone());
            
            if let Some(selected_sample) = &self.selected_sample {
                match self.audio_player.load(&selected_sample.path) {
                    Ok(_) => {
                        self.audio_player.play();
                    }
                    Err(error) => {
                        eprintln!("Error loading audio: {}", error);
                    }
                }
            }
        }
    }

    fn select_next_sample(&mut self) {
        let next_idx = match self.selected_sample_idx {
            Some(idx) => (idx + 1).min(self.samples.len().saturating_sub(1)),
            None => 0,
        };
        self.select_sample(next_idx);
    }

    fn select_prev_sample(&mut self) {
        let prev_idx = match self.selected_sample_idx {
            Some(idx) if idx > 0 => idx - 1,
            _ => 0,
        };
        self.select_sample(prev_idx);
    }

    fn import_samples_from_dir(
        &self,
        dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && ["wav", "flac", "mp3", "ogg"].contains(&ext.to_lowercase().as_str())
            {
                let file_meta = Sample::process_file(&path)?;
                self.db.insert_sample(&file_meta)?;
                println!("Added: {:?}", file_meta.name);
            }
        }

        Ok(())
    }

    pub fn query_samples(&mut self) {
        self.samples = self.db.get_samples().unwrap();
    }
}

impl From<Sample> for SampleItem {
    fn from(sample: Sample) -> Self {
        SampleItem {
            id: sample.id as i32,
            name: sample.name.into(),
            path: sample.path.into(),
            rating: sample.rating as i32,
            format: sample.format.into(),
            sample_count: sample.sample_count as i32,
            sample_rate: sample.sample_rate as i32,
            size: format_bytes(sample.size).into(),
            notes: sample.notes.into(),
        }
    }
}
