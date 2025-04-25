mod bars;
mod colour;
mod smoothing;
mod visualiser;

use colour::ChromagramColour;
use visualiser::VisualiserBuilder;

use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use windowfunctions::{Symmetry, WindowFunction, window};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const SAMPLE_RATE: usize = 44_100;
const FFT_SIZE: usize = 2048;
const FRAME_RATE: usize = 60;

fn get_audio_source() -> Simple {
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: 2,
        rate: SAMPLE_RATE as u32,
    };
    assert!(spec.is_valid());
    // Set lower latency (smaller buffer size)
    let buffer_attr = pulse::def::BufferAttr {
        maxlength: u32::MAX, // Let PulseAudio decide max size
        tlength: u32::MAX,   // Only used for playback
        prebuf: u32::MAX,    // Only used for playback
        minreq: u32::MAX,    // Only used for playback
        fragsize: 1024,      // Lower = lower latency (used for recording)
    };

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    Simple::new(
        None,               // Use the default server
        "AudioVisualiser",  // Our application's name
        Direction::Record,  // We want a recording stream
        Some(source_name),  // Use a monitor source
        "Audio Monitor",    // Description of our stream
        &spec,              // Our sample format
        None,               // Use default channel map
        Some(&buffer_attr), // Use default buffering attributes
    )
    .unwrap()
}

fn spawn_audio_reader(buffer: Arc<Mutex<VecDeque<f32>>>) {
    thread::spawn(move || {
        let mut raw_samples = [0u8; FFT_SIZE * 8]; // 8 bytes per stereo frame (2x f32)

        let s = get_audio_source();

        loop {
            if s.read(&mut raw_samples).is_ok() {
                let mut new_samples = Vec::with_capacity(FFT_SIZE);

                for chunk in raw_samples.chunks_exact(8) {
                    let left = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let right = f32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                    new_samples.push((left + right) / 2.0); // Mono
                }

                let mut buf = buffer.lock().unwrap();
                for s in new_samples {
                    buf.push_back(s);
                }

                // Trim the buffer to stay within the max size
                while buf.len() > FFT_SIZE {
                    buf.pop_front();
                }
            } else {
                eprintln!("Failed to read from audio source");
            }
        }
    });
}

/// Computes a single FFT on a buffer of audio samples
///
/// Note that only the first half of the values will be real frequencies
fn compute_fft(signal: &[f32], fft: &Arc<dyn rustfft::Fft<f32>>) -> Vec<f32> {
    let mut complex_samples: Vec<Complex<f32>> =
        signal.iter().map(|&v| Complex { re: v, im: 0.0 }).collect();

    fft.process(&mut complex_samples);

    // Convert to magnitudes
    let magnitudes: Vec<f32> = complex_samples.iter().map(|c| c.norm().powf(2.0)).collect();

    magnitudes
}

#[allow(dead_code)]
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (f32, f32, f32) {
    let i = (hue * 6.0).floor() as i32;
    let f = hue * 6.0 - i as f32;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - f * saturation);
    let t = value * (1.0 - (1.0 - f) * saturation);

    match i % 6 {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        5 => (value, p, q),
        _ => (0.0, 0.0, 0.0),
    }
}

async fn run_bar_visualiser(samples: Arc<Mutex<VecDeque<f32>>>) {
    // Visualiser setup
    let mut visualiser = VisualiserBuilder::new()
        .with_bars(bars::Bars::Normal { num_bars: 32 })
        .with_colour_mapper(Box::new(ChromagramColour::new(0.8f32)))
        .build(SAMPLE_RATE, FFT_SIZE);

    // For fixing visualiser FPS
    let mut last_frame_time = 0.0;
    let target_frame_duration = 1.0 / (FRAME_RATE as f64);

    // FFT setup
    let mut planner = FftPlanner::<f32>::new();
    let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(FFT_SIZE);

    // Hamming window to apply pre-FFT
    let window_type = WindowFunction::Hamming;
    let symmetry = Symmetry::Symmetric;
    let window_iter = window::<f32>(FFT_SIZE, window_type, symmetry);
    let window_vec: Vec<f32> = window_iter.into_iter().collect();

    loop {
        let current_time = macroquad::prelude::get_time();
        let frame_time = current_time - last_frame_time;

        clear_background(Color {
            r: 0.1,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        });

        let samples_to_use: Vec<f32> = samples
            .lock()
            .unwrap()
            .clone()
            .iter()
            .zip(&window_vec)
            .map(|(&x, &w)| x * w)
            .collect();

        if samples_to_use.len() < FFT_SIZE {
            next_frame().await;
            continue;
        }

        let spectrum = compute_fft(&samples_to_use, &fft);
        visualiser.update(&spectrum);
        last_frame_time = current_time;

        if frame_time < target_frame_duration {
            let sleep_duration = (target_frame_duration - frame_time) as u64 * 1_000;
            std::thread::sleep(std::time::Duration::from_millis(sleep_duration));
        }

        next_frame().await
    }
}

#[macroquad::main("Audio Visualiser")]
async fn main() {
    let shared_buffer: Arc<Mutex<VecDeque<f32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(FFT_SIZE)));

    spawn_audio_reader(shared_buffer.clone());

    run_bar_visualiser(shared_buffer.clone()).await;
}
