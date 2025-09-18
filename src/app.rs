use rusqlite::Connection;

use crate::{audio_player::AudioPlayer, db::{init_db, load_samples}, import_samples_from_dir, sample::Sample};

pub struct SampleDuckApp {
    pub conn: Connection,
    pub audio_player: AudioPlayer,
    pub samples: Vec<Sample>,
    pub selected_sample: Sample,
    pub selected_sample_idx: usize,
}

impl SampleDuckApp {
    pub fn new() -> Self {
        let conn = Connection::open("samples.db").expect("failed to open db");
        init_db(&conn).expect("failed to init db");

        // For now, scan a hardcoded folder
        import_samples_from_dir(&conn, "./demo/samples").unwrap();

        let mut audio_player = AudioPlayer::new().unwrap();
        let samples = load_samples(&conn).unwrap();

        let selected_sample_idx = 0;
        let selected_sample = samples[selected_sample_idx].clone();

        audio_player.load(&selected_sample.path);

        Self {
            conn,
            audio_player,
            samples,
            selected_sample,
            selected_sample_idx,
        }
    }
}
