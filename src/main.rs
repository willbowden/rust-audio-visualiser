use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const BUFFER_SIZE: usize = 4096; // e.g. ~100ms @ 44.1kHz mono
const CHUNK_SIZE: usize = 1024;

#[allow(dead_code)]
fn generate_sinusoid(
    buffer: &mut Vec<f32>,
    frequency: f32,
    sampling_rate: f32,
    amplitude: f32,
    duration: f32,
) {
    let num_samples = (sampling_rate * duration) as usize;
    let dt = 1.0 / sampling_rate;

    for i in 0..num_samples {
        let t = i as f32 * dt;
        buffer.push(amplitude * f32::sin(2.0 * PI * frequency * t));
    }
}

fn compute_fft(samples: &[f32], fft: &Arc<dyn rustfft::Fft<f32>>) -> Vec<f32> {
    let fft_size = samples.len(); // e.g., 512

    // Step 1: Convert mono samples to Complex<f32>
    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .map(|&s| Complex { re: s, im: 0.0 })
        .collect();

    // Step 2: Perform FFT
    fft.process(&mut buffer);

    // Step 3: Convert to magnitudes
    let magnitudes: Vec<f32> = buffer
        .iter()
        .take(fft_size / 2) // only keep first half (real frequencies)
        .map(|c| c.norm())
        .collect();

    magnitudes
}

fn get_audio_source() -> Simple {
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: 2,
        rate: 44100,
    };
    assert!(spec.is_valid());

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    Simple::new(
        None,              // Use the default server
        "AudioVisualiser", // Our application's name
        Direction::Record, // We want a recording stream
        Some(source_name), // Use a monitor source
        "Audio Monitor",   // Description of our stream
        &spec,             // Our sample format
        None,              // Use default channel map
        None,              // Use default buffering attributes
    )
    .unwrap()
}

fn spawn_audio_reader(buffer: Arc<Mutex<VecDeque<f32>>>) {
    thread::spawn(move || {
        let mut raw_samples = [0u8; CHUNK_SIZE * 8]; // 8 bytes per stereo frame (2x f32)

        let s = get_audio_source();

        loop {
            if let Ok(_) = s.read(&mut raw_samples) {
                let mut new_samples = Vec::with_capacity(CHUNK_SIZE);

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
                while buf.len() > BUFFER_SIZE {
                    buf.pop_front();
                }
            } else {
                eprintln!("Failed to read from audio source");
            }
        }
    });
}

async fn run_visualiser(samples: Arc<Mutex<VecDeque<f32>>>) {
    let bins = CHUNK_SIZE / 2;
    let bar_width: f32 = (screen_width() - 10.0) / (bins as f32);
    let max_height: f32 = screen_height() - 50.0;
    let bar_spacing: f32 = bar_width / 10.0;

    let mut last_frame_time = 0.0;
    let target_frame_duration = 1.0 / 60.0;

    let mut planner = FftPlanner::<f32>::new();
    let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(CHUNK_SIZE);

    loop {
        let current_time = macroquad::prelude::get_time();
        let frame_time = current_time - last_frame_time;

        clear_background(GRAY);

        let mut samples_to_use = samples
            .lock()
            .unwrap()
            .iter()
            .rev() // get most recent first
            .take(CHUNK_SIZE)
            .cloned()
            .collect::<Vec<_>>();
        samples_to_use.reverse(); // reverse back to time order

        if samples_to_use.len() < CHUNK_SIZE {
            next_frame().await;
            continue;
        }

        let spectrum = compute_fft(&samples_to_use, &fft);

        let spectrum_log: Vec<f32> = spectrum
            .iter()
            .map(|m| (1.0 + *m).log10())
            .collect();

        let max_val = spectrum_log.iter().cloned().fold(0.0, f32::max);
        let normalised: Vec<f32> = spectrum_log.iter().map(|m| m / max_val).collect();


        for (i, ampl) in normalised.iter().enumerate() {
            let index = i as f32;
            let bar_height = ampl * max_height;
            let x = (index * bar_width) + (index * bar_spacing) + bar_spacing;
            let y = screen_height() - bar_height - 10.0;

            draw_rectangle(x, y, bar_width, bar_height, WHITE);
        }

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
        Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_SIZE)));

    spawn_audio_reader(shared_buffer.clone());

    run_visualiser(shared_buffer.clone()).await;
}
