use std::fs::File;

use symphonia::{
    core::{
        audio::{AudioBufferRef, Signal, SignalSpec},
        codecs::{CODEC_TYPE_NULL, DecoderOptions},
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
    },
    default::{get_codecs, get_probe},
};

use crate::audio_player::AudioPlayerError;
pub fn load_file(path: &str, out_channels: usize) -> Result<Vec<f32>, AudioPlayerError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // 2. Probe file format
    let probed = get_probe()
        .format(
            &Default::default(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| AudioPlayerError::SymphoniaError(Box::new(e)))?;

    let mut format = probed.format;

    // 3. Find first supported audio track
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

    // 4. Create decoder
    let mut decoder = get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AudioPlayerError::SymphoniaError(Box::new(e)))?;

    // 6. Decode all packets into a Vec<f32>
    decode_all_packets(format.as_mut(), decoder.as_mut(), out_channels)
}

fn decode_all_packets(
    format: &mut dyn symphonia::core::formats::FormatReader,
    decoder: &mut dyn symphonia::core::codecs::Decoder,
    out_channels: usize,
) -> Result<Vec<f32>, AudioPlayerError> {
    let mut samples = Vec::new();
    let mut packet_count = 0;

    while let Ok(packet) = format.next_packet() {
        packet_count += 1;

        let decoded = decoder.decode(&packet).map_err(|e| {
            AudioPlayerError::DecodingError(format!(
                "Failed to decode packet {}: {}",
                packet_count, e
            ))
        })?;

        let before_len = samples.len();
        process_audio_buffer(decoded, out_channels, &mut samples)?;
        let added_samples = samples.len() - before_len;

        if packet_count <= 5 || packet_count % 100 == 0 {
            println!(
                "Processed packet {}: added {} samples (total: {})",
                packet_count,
                added_samples,
                samples.len()
            );
        }
    }

    if samples.is_empty() {
        return Err(AudioPlayerError::DecodingError(
            "No audio samples decoded".to_string(),
        ));
    }

    Ok(samples)
}

fn process_audio_buffer(
    decoded: AudioBufferRef,
    out_channels: usize,
    output: &mut Vec<f32>,
) -> Result<(), AudioPlayerError> {
    match decoded {
        AudioBufferRef::F32(buf) => {
            let ch1 = if buf.spec().channels.count() > 1 {
                buf.chan(1)
            } else {
                &[]
            };
            convert_buffer(buf.chan(0), ch1, *buf.spec(), out_channels, output);
        }
        AudioBufferRef::F64(buf) => {
            let ch0: Vec<f32> = buf.chan(0).iter().map(|&s| s as f32).collect();
            let ch1: Vec<f32> = if buf.spec().channels.count() > 1 {
                buf.chan(1).iter().map(|&s| s as f32).collect()
            } else {
                Vec::new()
            };
            convert_buffer(&ch0, &ch1, *buf.spec(), out_channels, output);
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
            convert_buffer(&ch0, &ch1, *buf.spec(), out_channels, output);
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
            convert_buffer(&ch0, &ch1, *buf.spec(), out_channels, output);
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
            convert_buffer(&ch0, &ch1, *buf.spec(), out_channels, output);
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
            convert_buffer(&ch0, &ch1, *buf.spec(), out_channels, output);
        }
        _ => {
            return Err(AudioPlayerError::UnsupportedFormat(
                "Unsupported audio buffer format".to_string(),
            ));
        }
    }
    Ok(())
}

fn convert_buffer(
    ch0: &[f32],
    ch1: &[f32],
    spec: SignalSpec,
    out_channels: usize,
    output: &mut Vec<f32>,
) {
    let in_channels = spec.channels.count();
    let out_channels = out_channels;

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

pub fn format_bytes(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let size_f = size as f64;

    if size_f < KB {
        format!("{} B", size)
    } else if size_f < MB {
        format!("{:.2} KB", size_f / KB)
    } else if size_f < GB {
        format!("{:.2} MB", size_f / MB)
    } else if size_f < TB {
        format!("{:.2} GB", size_f / GB)
    } else {
        format!("{:.2} TB", size_f / TB)
    }
}
