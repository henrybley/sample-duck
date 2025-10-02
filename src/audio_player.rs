use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio_file::load_file;

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

        self.stop();

        let new_samples = load_file(path, self.out_channels)?;

        println!("Successfully loaded {} samples", new_samples.len());

        // 7. Update player state
        self.samples_count = new_samples.len();
        *self.samples.lock().unwrap() = new_samples;
        self.play_pos.store(0, Ordering::Relaxed);
        *self.state.lock().unwrap() = PlaybackState::Stopped;

        // 8. Compute peaks for visualization
        self.peak_samples = AudioPlayer::compute_peaks(&*self.samples.lock().unwrap());

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
                self.pause();
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
