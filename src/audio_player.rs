use std::error::Error;
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use symphonia::core::audio::{AudioBufferRef, Signal, SignalSpec};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};

#[derive(Debug)]
pub enum AudioPlayerError {
    NoOutputDevice,
    UnsupportedFormat(String),
    DecodingError(String),
    IoError(std::io::Error),
    CpalBuildStreamError(cpal::BuildStreamError),
    CpalDefaultStreamConfigError(cpal::DefaultStreamConfigError),
    CpalPlayStreamError(cpal::PlayStreamError),
    SymphoniaError(Box<dyn Error>),
}

impl fmt::Display for AudioPlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioPlayerError::NoOutputDevice => write!(f, "No audio output device available"),
            AudioPlayerError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
            AudioPlayerError::DecodingError(msg) => write!(f, "Decoding error: {}", msg),
            AudioPlayerError::IoError(err) => write!(f, "IO error: {}", err),
            AudioPlayerError::CpalBuildStreamError(err) => {
                write!(f, "CPAL build stream error: {}", err)
            }
            AudioPlayerError::CpalDefaultStreamConfigError(err) => {
                write!(f, "CPAL default config error: {}", err)
            }
            AudioPlayerError::CpalPlayStreamError(err) => {
                write!(f, "CPAL play stream error: {}", err)
            }
            AudioPlayerError::SymphoniaError(err) => write!(f, "Symphonia error: {}", err),
        }
    }
}

impl Error for AudioPlayerError {}

impl From<std::io::Error> for AudioPlayerError {
    fn from(err: std::io::Error) -> Self {
        AudioPlayerError::IoError(err)
    }
}

impl From<cpal::BuildStreamError> for AudioPlayerError {
    fn from(err: cpal::BuildStreamError) -> Self {
        AudioPlayerError::CpalBuildStreamError(err)
    }
}

impl From<cpal::DefaultStreamConfigError> for AudioPlayerError {
    fn from(err: cpal::DefaultStreamConfigError) -> Self {
        AudioPlayerError::CpalDefaultStreamConfigError(err)
    }
}

impl From<cpal::PlayStreamError> for AudioPlayerError {
    fn from(err: cpal::PlayStreamError) -> Self {
        AudioPlayerError::CpalPlayStreamError(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct AudioPlayer {
    samples: Arc<Mutex<Vec<f32>>>,
    pub samples_count: usize,
    pub peak_samples: Vec<(f32, f32)>,
    play_pos: Arc<AtomicUsize>,
    state: Arc<Mutex<PlaybackState>>,
    _stream: cpal::Stream,
    out_channels: usize,
    sample_rate: u32,
    loop_enabled: Arc<Mutex<bool>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, AudioPlayerError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioPlayerError::NoOutputDevice)?;

        let config = device.default_output_config()?.config();
        let out_channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;

        let samples = Arc::new(Mutex::new(Vec::new()));
        let play_pos = Arc::new(AtomicUsize::new(0));
        let state = Arc::new(Mutex::new(PlaybackState::Stopped));
        let loop_enabled = Arc::new(Mutex::new(false));

        // Clone for callback
        let samples_cb = Arc::clone(&samples);
        let play_pos_cb = Arc::clone(&play_pos);
        let state_cb = Arc::clone(&state);
        let loop_enabled_cb = Arc::clone(&loop_enabled);

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                Self::audio_callback(
                    data,
                    &samples_cb,
                    &play_pos_cb,
                    &state_cb,
                    &loop_enabled_cb,
                    out_channels,
                );
            },
            move |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;

        let peak_samples: Vec<(f32, f32)> = vec![];

        Ok(Self {
            samples,
            samples_count: 0,
            peak_samples,
            play_pos,
            state,
            _stream: stream,
            out_channels,
            sample_rate,
            loop_enabled,
        })
    }

    fn audio_callback(
        data: &mut [f32],
        samples: &Arc<Mutex<Vec<f32>>>,
        play_pos: &Arc<AtomicUsize>,
        state: &Arc<Mutex<PlaybackState>>,
        loop_enabled: &Arc<Mutex<bool>>,
        out_channels: usize,
    ) {
        let samples_guard = samples.lock().unwrap();
        let current_state = *state.lock().unwrap();
        let is_looping = *loop_enabled.lock().unwrap();

        // Clear output buffer first
        data.fill(0.0);

        if current_state != PlaybackState::Playing || samples_guard.is_empty() {
            return;
        }

        let mut pos = play_pos.load(Ordering::Relaxed);

        for frame in data.chunks_mut(out_channels) {
            if pos + out_channels <= samples_guard.len() {
                frame.copy_from_slice(&samples_guard[pos..pos + out_channels]);
                pos += out_channels;
            } else if is_looping {
                // Loop back to beginning
                pos = 0;
                if out_channels <= samples_guard.len() {
                    frame.copy_from_slice(&samples_guard[0..out_channels]);
                    pos += out_channels;
                }
            } else {
                // End of playback
                *state.lock().unwrap() = PlaybackState::Stopped;
                break;
            }
        }

        // Write back updated position
        play_pos.store(pos, Ordering::Relaxed);
    }

    pub fn load(&mut self, path: &str) -> Result<(), AudioPlayerError> {
        println!("Loading audio file: {}", path);

        let file = File::open(Path::new(path))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Probe the file format
        let probed = get_probe()
            .format(
                &Default::default(),
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| AudioPlayerError::SymphoniaError(Box::new(e)))?;

        let mut format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| {
                AudioPlayerError::UnsupportedFormat("No supported audio tracks found".to_string())
            })?;

        println!(
            "Track info: codec={:?}, channels={:?}, sample_rate={:?}",
            track.codec_params.codec, track.codec_params.channels, track.codec_params.sample_rate
        );

        // Create decoder
        let mut decoder = get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| AudioPlayerError::SymphoniaError(Box::new(e)))?;

        // Stop current playback
        self.stop();

        let mut new_samples = Vec::new();
        let mut packet_count = 0;

        // Decode all packets
        while let Ok(packet) = format.next_packet() {
            packet_count += 1;

            let decoded = decoder.decode(&packet).map_err(|e| {
                AudioPlayerError::DecodingError(format!(
                    "Failed to decode packet {}: {}",
                    packet_count, e
                ))
            })?;

            let before_len = new_samples.len();
            self.process_audio_buffer(decoded, &mut new_samples)?;
            let added_samples = new_samples.len() - before_len;

            if packet_count <= 5 || packet_count % 100 == 0 {
                println!(
                    "Processed packet {}: added {} samples (total: {})",
                    packet_count,
                    added_samples,
                    new_samples.len()
                );
            }
        }

        if new_samples.is_empty() {
            return Err(AudioPlayerError::DecodingError(
                "No audio samples decoded".to_string(),
            ));
        }

        println!(
            "Successfully loaded {} samples from {} packets",
            new_samples.len(),
            packet_count
        );

        // Update player state
        self.samples_count = new_samples.len();
        *self.samples.lock().unwrap() = new_samples;
        self.play_pos.store(0, Ordering::Relaxed);
        *self.state.lock().unwrap() = PlaybackState::Stopped;

        self.peak_samples = Self::compute_peaks(&*self.samples.lock().unwrap());

        Ok(())
    }

    fn process_audio_buffer(
        &self,
        decoded: AudioBufferRef,
        output: &mut Vec<f32>,
    ) -> Result<(), AudioPlayerError> {
        match decoded {
            AudioBufferRef::F32(buf) => {
                let ch1 = if buf.spec().channels.count() > 1 {
                    buf.chan(1)
                } else {
                    &[]
                };
                self.convert_buffer(buf.chan(0), ch1, *buf.spec(), output);
            }
            AudioBufferRef::F64(buf) => {
                let ch0: Vec<f32> = buf.chan(0).iter().map(|&s| s as f32).collect();
                let ch1: Vec<f32> = if buf.spec().channels.count() > 1 {
                    buf.chan(1).iter().map(|&s| s as f32).collect()
                } else {
                    Vec::new()
                };
                self.convert_buffer(&ch0, &ch1, *buf.spec(), output);
            }
            AudioBufferRef::S16(buf) => {
                let ch0: Vec<f32> = buf
                    .chan(0)
                    .iter()
                    .map(|&s| s as f32 / i16::MAX as f32)
                    .collect();
                let ch1: Vec<f32> = if buf.spec().channels.count() > 1 {
                    buf.chan(1)
                        .iter()
                        .map(|&s| s as f32 / i16::MAX as f32)
                        .collect()
                } else {
                    Vec::new()
                };
                self.convert_buffer(&ch0, &ch1, *buf.spec(), output);
            }
            AudioBufferRef::S32(buf) => {
                let ch0: Vec<f32> = buf
                    .chan(0)
                    .iter()
                    .map(|&s| s as f32 / i32::MAX as f32)
                    .collect();
                let ch1: Vec<f32> = if buf.spec().channels.count() > 1 {
                    buf.chan(1)
                        .iter()
                        .map(|&s| s as f32 / i32::MAX as f32)
                        .collect()
                } else {
                    Vec::new()
                };
                self.convert_buffer(&ch0, &ch1, *buf.spec(), output);
            }
            AudioBufferRef::S24(buf) => {
                // Fixed S24 normalization
                let ch0: Vec<f32> = buf
                    .chan(0)
                    .iter()
                    .map(|&s| {
                        let i32_val = s.inner();
                        // Proper S24 normalization: signed 24-bit has range [-2^23, 2^23-1]
                        if i32_val >= 0 {
                            i32_val as f32 / 8_388_607.0 // 2^23 - 1
                        } else {
                            i32_val as f32 / 8_388_608.0 // 2^23
                        }
                    })
                    .collect();
                let ch1: Vec<f32> = if buf.spec().channels.count() > 1 {
                    buf.chan(1)
                        .iter()
                        .map(|&s| {
                            let i32_val = s.inner();
                            if i32_val >= 0 {
                                i32_val as f32 / 8_388_607.0
                            } else {
                                i32_val as f32 / 8_388_608.0
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                self.convert_buffer(&ch0, &ch1, *buf.spec(), output);
            }
            AudioBufferRef::U8(buf) => {
                let ch0: Vec<f32> = buf
                    .chan(0)
                    .iter()
                    .map(|&s| (s as f32 - 128.0) / 128.0)
                    .collect();
                let ch1: Vec<f32> = if buf.spec().channels.count() > 1 {
                    buf.chan(1)
                        .iter()
                        .map(|&s| (s as f32 - 128.0) / 128.0)
                        .collect()
                } else {
                    Vec::new()
                };
                self.convert_buffer(&ch0, &ch1, *buf.spec(), output);
            }
            _ => {
                return Err(AudioPlayerError::UnsupportedFormat(
                    "Unsupported audio buffer format".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn compute_peaks(samples: &[f32]) -> Vec<(f32, f32)> {
        // Create peak samples for efficient visualization
        // We'll downsample to have ~2000 points for display
        let target_points = 2000;
        let samples_per_point = samples.len().max(target_points) / target_points;
        let mut peak_samples = Vec::with_capacity(target_points);

        for chunk in samples.chunks(samples_per_point) {
            if !chunk.is_empty() {
                let min = *chunk
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(&0.0);
                let max = *chunk
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(&0.0);
                peak_samples.push((min, max));
            }
        }

        peak_samples
    }

    fn convert_buffer(&self, ch0: &[f32], ch1: &[f32], spec: SignalSpec, output: &mut Vec<f32>) {
        let in_channels = spec.channels.count();
        let out_channels = self.out_channels;

        match (in_channels, out_channels) {
            (1, 1) => {
                output.extend_from_slice(ch0);
            }
            (1, 2) => {
                // Mono to stereo: duplicate mono channel
                for &sample in ch0 {
                    output.push(sample);
                    output.push(sample);
                }
            }
            (2, 1) => {
                // Stereo to mono: mix both channels
                for i in 0..ch0.len() {
                    let left = ch0[i];
                    let right = ch1.get(i).copied().unwrap_or(0.0);
                    output.push((left + right) * 0.5);
                }
            }
            (2, 2) => {
                // Stereo to stereo: direct copy
                for i in 0..ch0.len() {
                    output.push(ch0[i]);
                    output.push(ch1.get(i).copied().unwrap_or(0.0));
                }
            }
            (n, m) => {
                println!(
                    "Warning: Unusual channel configuration {} -> {}, using fallback",
                    n, m
                );
                // Fallback: duplicate first channel for all output channels
                for &sample in ch0 {
                    for _ in 0..out_channels {
                        output.push(sample);
                    }
                }
            }
        }
    }

    // Playback control methods
    pub fn play(&self) {
        *self.state.lock().unwrap() = PlaybackState::Playing;
        println!("Playback started");
    }

    pub fn pause(&self) {
        *self.state.lock().unwrap() = PlaybackState::Paused;
        println!("Playback paused");
    }

    pub fn stop(&self) {
        *self.state.lock().unwrap() = PlaybackState::Stopped;
        self.play_pos.store(0, Ordering::Relaxed);
        println!("Playback stopped");
    }

    pub fn toggle_play_state(&mut self) {
        match self.get_state() {
            PlaybackState::Stopped => {
                self.play();
            }
            PlaybackState::Paused => {
                self.play();
            }
            PlaybackState::Playing => {
                self.stop();
            }
        }
    }

    pub fn set_loop(&self, enabled: bool) {
        *self.loop_enabled.lock().unwrap() = enabled;
        println!("Loop {}", if enabled { "enabled" } else { "disabled" });
    }

    pub fn get_state(&self) -> PlaybackState {
        *self.state.lock().unwrap()
    }

    pub fn get_position_index(&self) -> usize {
        self.play_pos.load(Ordering::Relaxed)
    }

    pub fn get_position_percentage(&self) -> f32 {
        self.get_position_index() as f32 / self.samples_count as f32
    }

    pub fn seek_to_position_percentage(&self, sample_pos_percent: f32) {
        self.seek_to_position((self.samples_count as f32 * sample_pos_percent) as usize);
    }

    pub fn seek_to_position(&self, sample_pos: usize) {
        let total_samples = self.samples.lock().unwrap().len();
        let clamped_pos = sample_pos.min(total_samples);
        self.play_pos.store(clamped_pos, Ordering::Relaxed);
        println!("Position set to sample {}/{}", clamped_pos, total_samples);
    }

    pub fn get_duration_seconds(&self) -> f32 {
        let total_samples = self.samples.lock().unwrap().len();
        let frames = total_samples / self.out_channels;
        frames as f32 / self.sample_rate as f32
    }
}
