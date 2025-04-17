use macroquad::miniquad::log;
use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use windowfunctions::{Symmetry, WindowFunction, window};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const SAMPLE_RATE: usize = 44_100;
const BUFFER_SIZE: usize = 4096; // e.g. ~100ms @ 44.1kHz mono
const FFT_SIZE: usize = 1024;
const BAR_COUNT: usize = 128;

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

fn make_log_bins(
    fft_size: usize,
    num_bins: usize,
    sample_rate: usize,
) -> Vec<(usize, usize)> {
    let nyquist = sample_rate as f32 / 2.0; // Max frequency we can represent
    let max_bin = fft_size / 2;

    // Logarithmically spaced frequency bands
    let log_start = (1.0_f32).ln();
    let log_end = (nyquist).ln();

    let mut ranges = Vec::new();

    for i in 0..num_bins {
        let fraction = i as f32 / num_bins as f32;

        let freq_start = (log_start * (1.0 - fraction) + log_end * fraction).exp();
        let freq_end = (log_start * (1.0 - (fraction + 1.0 / num_bins as f32))
            + log_end * (fraction + 1.0 / num_bins as f32))
            .exp();

        let bin_start = ((freq_start / nyquist) * max_bin as f32).floor() as usize;
        let bin_end = ((freq_end / nyquist) * max_bin as f32).ceil() as usize;

        
        if bin_start < bin_end {
            println!("Bin {} from frequency {}Hz to {}Hz occupies index range ({}, {})", i, freq_start, freq_end, bin_start, bin_end);
            ranges.push((bin_start,bin_end));
        }
    }

    ranges
}

fn get_audio_source() -> Simple {
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: 2,
        rate: SAMPLE_RATE as u32,
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
        let mut raw_samples = [0u8; FFT_SIZE * 8]; // 8 bytes per stereo frame (2x f32)

        let s = get_audio_source();

        loop {
            if let Ok(_) = s.read(&mut raw_samples) {
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
    // Rendering parameters
    let bar_width: f32 = (screen_width() - 10.0) / (BAR_COUNT as f32);
    let max_height: f32 = screen_height() - 50.0;
    let bar_spacing: f32 = bar_width / 10.0;

    // For fixing visualiser FPS
    let mut last_frame_time = 0.0;
    let target_frame_duration = 1.0 / 60.0;

    // FFT setup
    let mut planner = FftPlanner::<f32>::new();
    let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(FFT_SIZE);

    // Hamming window to apply pre-FFT
    let window_type = WindowFunction::Hamming;
    let symmetry = Symmetry::Symmetric;
    let window_iter = window::<f32>(FFT_SIZE, window_type, symmetry);
    let window_vec: Vec<f32> = window_iter.into_iter().collect();

    let bin_ranges = make_log_bins(FFT_SIZE, BAR_COUNT, SAMPLE_RATE);

    loop {
        let current_time = macroquad::prelude::get_time();
        let frame_time = current_time - last_frame_time;

        clear_background(GRAY);

        let mut samples_to_use = samples
            .lock()
            .unwrap()
            .iter()
            .rev() // get most recent first
            .take(FFT_SIZE)
            .cloned()
            .collect::<Vec<_>>();

        samples_to_use.reverse(); // reverse back to time order

        if samples_to_use.len() < FFT_SIZE {
            next_frame().await;
            continue;
        }

        // Apply windowing function
        for i in 0..samples_to_use.len() {
            samples_to_use[i] = samples_to_use[i] * window_vec[i];
        }

        // Compute FFT
        let spectrum = compute_fft(&samples_to_use, &fft);

        let log_bars: Vec<f32> = bin_ranges
            .iter()
            .map(|(start, end)| {
                if *end > spectrum.len() {
                    return 0.0;
                }
                let slice = &spectrum[*start..*end];
                slice.iter().copied().fold(0.0, f32::max)
            })
            .collect();

        let spectrum_log: Vec<f32> = log_bars.iter().map(|m| (1.0 + *m).log10()).collect();

        let max_val = spectrum_log.iter().cloned().fold(0.0, f32::max);
        let normalised: Vec<f32> = spectrum_log.iter().map(|m| m / max_val).collect();

        for (i, ampl) in normalised.iter().skip(1).enumerate() {
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
