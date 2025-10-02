use std::{fs::File, path::Path};

use symphonia::{
    core::{formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions},
    default::get_probe,
};

use crate::audio_file::load_file;

#[derive(Debug, Clone)]
pub struct Sample {
    pub id: isize,
    pub path: String,
    pub name: String,
    pub format: String,
    pub sample_count: u32,
    pub sample_rate: u32,
    pub key: String,
    pub size: u64,
    pub rating: u8,
    pub notes: String,
}

impl Sample {
    pub fn process_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
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

        let format = probed.format;

        // Take the first audio track
        let track = &format.tracks()[0];
        let codec_params = &track.codec_params;

        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let format_name = codec_params.codec.to_string();

        let samples = load_file(path.to_str().unwrap(), 1)?;

        Ok(Sample {
            id: 0,
            path: path.to_string_lossy().to_string(),
            name,
            format: format_name,
            sample_count: samples.len() as u32,
            sample_rate,
            key: "".to_string(),
            size,
            rating: 0,
            notes: "".to_string(),
        })
    }
}
