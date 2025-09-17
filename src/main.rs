use std::fs;
use std::{fs::File, path::Path};

use rusqlite::Connection;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::get_probe;

use crate::app::SampleDuckApp;
use crate::db::insert_sample;
use crate::sample::Sample;

mod app;
mod audio_player;
mod db;
mod sample;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Sample Manager",
        options,
        Box::new(|_cc| Ok(Box::new(SampleDuckApp::new()))),
    )
}

fn import_samples_from_dir(conn: &Connection, dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && ["wav", "flac", "mp3", "ogg"].contains(&ext.to_lowercase().as_str())
        {
            let file_meta = process_file(&path)?;
            insert_sample(conn, &file_meta)?;
            println!("Added: {:?}", file_meta.name);
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
