use crate::{audio_player::AudioPlayer, db::DB, sample::Sample};
use std::{fs, path::Path};

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

    pub(crate) fn import_samples_from_dir(&self, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
