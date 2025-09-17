use egui_extras::{Column, TableBuilder};
use std::fs;
use std::{fs::File, path::Path};

use eframe::egui;
use rusqlite::{Connection, params};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::get_probe;

use crate::audio_player::AudioPlayer;

mod audio_player;

#[derive(Debug, Clone)]
struct Sample {
    id: isize,
    path: String,
    name: String,
    format: String,
    sample_rate: u32,
    size: u64,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Sample Manager",
        options,
        Box::new(|_cc| Ok(Box::new(SampleDuckApp::new()))),
    )
}

struct SampleDuckApp {
    conn: Connection,
    audio_player: AudioPlayer,
    samples: Vec<Sample>,
    selected_sample: Sample,
    selected_sample_idx: usize,
}

impl SampleDuckApp {
    fn new() -> Self {
        let conn = Connection::open("samples.db").expect("failed to open db");
        init_db(&conn).expect("failed to init db");

        // For now, scan a hardcoded folder
        import_samples_from_dir(&conn, "./demo/samples").unwrap();

        let audio_player = AudioPlayer::new().unwrap();
        let samples = load_samples(&conn).unwrap();

        let selected_sample_idx = 0;
        let selected_sample = samples[selected_sample_idx].clone();

        Self {
            conn,
            audio_player,
            samples,
            selected_sample,
            selected_sample_idx,
        }
    }
    fn select_sample(&mut self, sample_idx: usize) {
        if self.samples.len() > sample_idx {
            self.selected_sample_idx = sample_idx;
            self.selected_sample = self.samples[sample_idx].clone();
            match self.audio_player.load(&self.selected_sample.path) {
                Ok(_) => {
                    self.audio_player.play();
                }
                Err(error) => {
                    println!("Error: {}", error);
                }
            }
        }
    }

    fn click_sample(&mut self, sample_idx: usize, row_response: &egui::Response) {
        if row_response.clicked() {
            self.select_sample(sample_idx);
        }
    }

    fn select_next_sample(&mut self) {
        self.select_sample(self.selected_sample_idx + 1);
    }

    fn select_prev_sample(&mut self) {
        if self.selected_sample_idx > 0 {
            self.select_sample(self.selected_sample_idx - 1);
        }
    }

    
}

impl eframe::App for SampleDuckApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.input(|i| i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown)) {
                self.select_next_sample();
            }
            if ui.input(|i| i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp)) {
                self.select_prev_sample();
            }

            if ui.input(|i| i.key_pressed(egui::Key::Space)) {
                self.audio_player.toggle_play_state();
            }

            ui.heading("Sample Manager");
            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_height(500.0);
                    ui.set_max_width(ui.available_width() - 270.0); // Reserve space for right panel + margin
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.set_width(ui.available_width() * 0.5);
                        self.sample_list(ui);
                    });
                });

                ui.vertical(|ui| {
                    let size = egui::Vec2::new(250.0, ui.available_height());
                    ui.allocate_ui(size, |ui| {
                        ui.heading("Sample");
                        ui.label(self.selected_sample.name.clone());
                    });
                });
            });
        });
    }
}
impl SampleDuckApp {
    fn sample_list(&mut self, ui: &mut egui::Ui) {
        let available_height = ui.available_height();

        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height);

        table = table.sense(egui::Sense::click());

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("ID");
                });
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Path");
                });
                header.col(|ui| {
                    ui.strong("Format");
                });
                header.col(|ui| {
                    ui.strong("Sample Rate");
                });
                header.col(|ui| {
                    ui.strong("Size");
                });
            })
            .body(|mut body| {
                for (idx, sample) in self.samples.clone().iter().enumerate() {
                    let row_height = 18.0;
                    body.row(row_height, |mut row| {
                        row.set_selected(self.selected_sample.id == sample.id);
                        row.col(|ui| {
                            ui.label(sample.id.to_string());
                        });
                        row.col(|ui| {
                            ui.label(sample.name.clone());
                        });
                        row.col(|ui| {
                            ui.label(sample.path.clone());
                        });
                        row.col(|ui| {
                            ui.label(sample.format.to_string());
                        });
                        row.col(|ui| {
                            ui.label(sample.sample_rate.to_string());
                        });
                        row.col(|ui| {
                            ui.label(sample.size.to_string());
                        });

                        self.click_sample(idx, &row.response());
                    });
                }
            });
    }
}

fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS samples (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            format TEXT,
            sample_rate INTEGER,
            size INTEGER
        );
        ",
    )?;
    Ok(())
}

fn import_samples_from_dir(conn: &Connection, dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ["wav", "flac", "mp3", "ogg"].contains(&ext.to_lowercase().as_str()) {
                let file_meta = process_file(&path)?;
                insert_sample(&conn, &file_meta)?;
                println!("Added: {:?}", file_meta.name);
            }
        }
    }

    Ok(())
}

fn process_file(path: &Path) -> Result<Sample, Box<dyn std::error::Error>> {
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let size = std::fs::metadata(path)?.len();

    // Open file
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Probe format
    let probed = get_probe().format(
        &Default::default(),
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let format_reader = probed.format;

    // Take the first audio track
    let track = &format_reader.tracks()[0];
    let codec_params = &track.codec_params;

    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let format_name = codec_params.codec.to_string();

    Ok(Sample {
        id: 0,
        path: path.to_string_lossy().to_string(),
        name,
        format: format_name,
        sample_rate,
        size,
    })
}

fn insert_sample(conn: &Connection, meta: &Sample) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO samples (path, name, format, sample_rate, size)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            meta.path,
            meta.name,
            meta.format,
            meta.sample_rate,
            meta.size as i64,
        ],
    )?;
    Ok(())
}

fn load_samples(conn: &Connection) -> rusqlite::Result<Vec<Sample>> {
    let mut stmt = conn.prepare("SELECT id, path, name, format, sample_rate, size FROM samples")?;
    let rows = stmt.query_map([], |row| {
        Ok(Sample {
            id: row.get(0)?,
            path: row.get(1)?,
            name: row.get(2)?,
            format: row.get(3)?,
            sample_rate: row.get(4)?,
            size: row.get(5)?,
        })
    })?;

    let mut samples = Vec::new();
    for row in rows {
        samples.push(row?);
    }
    Ok(samples)
}
