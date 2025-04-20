use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use windowfunctions::{Symmetry, WindowFunction, window};

use std::cmp::max;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const SAMPLE_RATE: usize = 44_100;
const FFT_SIZE: usize = 2048;
const FRAME_RATE: usize = 60;

/// Compute how to split an FFT of length `fft_size` into `num_bins` using common music frequency ranges
///
/// To be computed in advance and reused across FFT processes
fn log_ranges(num_bins: usize, sample_rate: usize, fft_size: usize) -> Vec<(usize, usize)> {
    let weights = [
        ("Sub-bass", 0.08),
        ("Bass", 0.16),
        ("Low Mids", 0.16),
        ("Mids", 0.26),
        ("Upper Mids", 0.22),
        ("Highs", 0.12),
    ];

    let freq_ranges: [(f32, f32); 6] = [
        (0.0, 60.0),
        (60.0, 250.0),
        (250.0, 500.0),
        (500.0, 2000.0),
        (2000.0, 6000.0),
        (6000.0, 20000.0),
    ];

    let freq_per_bin = sample_rate as f32 / fft_size as f32;

    let mut bins_per_range = weights.map(|(_, v)| (num_bins as f32 * v).floor() as usize);

    let mut bin_sum: usize = bins_per_range.iter().sum();
    let mut index = 0;

    while bin_sum < num_bins {
        bins_per_range[index] += 1;
        bin_sum += 1;
        index += 1;
    }

    let mut last_bin_end = 0;

    let mut ranges = Vec::new();

    for (i, &bin_count) in bins_per_range.iter().enumerate() {
        let (start, end) = freq_ranges[i];

        let log_start = start.log10();
        let log_end = end.log10();

        let step = (log_end - log_start) / bin_count as f32;

        for j in 0..bin_count {
            let f_low = 10.0_f32.powf(log_start + j as f32 * step);
            let f_high = 10.0_f32.powf(log_start + (j as f32 + 1.0) * step);

            let computed_bin_start = ((f_low / freq_per_bin) - 1.0).round() as usize;
            let computed_bin_end = ((f_high / freq_per_bin) - 1.0).round() as usize;

            let bin_start = max(computed_bin_start, last_bin_end);
            let bin_end = max(bin_start + 1, computed_bin_end); // Ensure at least 1 bin

            ranges.push((bin_start, bin_end));
            last_bin_end = bin_end;
        }
    }

    // for (i, &(start, stop)) in ranges.iter().enumerate() {
    //     println!("Frequency bin {}: {}Hz-{}Hz", i+1, ((start as f32) * freq_per_bin).round(), ((stop as f32) * freq_per_bin).round());
    // }

    ranges
}

/// Computes `num_bins` ranges for an FFT of size `fft_size` using gamma correction
fn gamma_corrected_ranges(
    num_bins: usize,
    sample_rate: usize,
    fft_size: usize,
    gamma: f32,
) -> Vec<(usize, usize)> {
    let nyquist = sample_rate as f32 / 2.0;
    let freq_per_bin = sample_rate as f32 / fft_size as f32;

    let mut ranges = Vec::new();

    let mut start: usize = 0;

    for i in 0..fft_size {
        let freq = i as f32 * freq_per_bin;
        let norm_freq = freq / nyquist;

        let b_i = (norm_freq.powf(1.0 / gamma) * ((num_bins) as f32).floor()) as usize;

        if b_i != start {
            // println!("Frequency {} is going in bar {}", freq, b_i);
            ranges.push((start, b_i));
            start = b_i;
        }
    }

    ranges
}

/// Converts an FFT spectrum into `num_bars` bars spaced based on predefined ranges`bar_ranges`
///
/// Averages and takes the log_2 of the values in each bar
fn take_log_mean_ranges(spectrum: &Vec<f32>, bar_ranges: &Vec<(usize, usize)>) -> Vec<f32> {
    let mut log_bars = vec![0.0; bar_ranges.len()];

    for (i, &(start, end)) in bar_ranges.iter().enumerate() {
        let slice: &[f32] = &spectrum[start..end];
        let sum: f32 = slice.iter().sum();
        log_bars[i] = ((sum / slice.len() as f32) + 1.0).log2();
    }

    log_bars
}

/// Converts an FFT spectrum into `num_bars` bars spaced based on predefined ranges`bar_ranges`
///
/// Averages and takes the log_2 of the values in each bar
fn take_log_max_ranges(spectrum: &Vec<f32>, bar_ranges: &Vec<(usize, usize)>) -> Vec<f32> {
    let mut log_bars = vec![0.0; bar_ranges.len()];

    for (i, &(start, end)) in bar_ranges.iter().enumerate() {
        let slice: &[f32] = &spectrum[start..end];
        let max_value: f32 = slice.iter().copied().fold(0.0, f32::max);
        log_bars[i] = (max_value + 1.0).log2();
    }

    log_bars
}

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
fn compute_fft(signal: &Vec<f32>, fft: &Arc<dyn rustfft::Fft<f32>>) -> Vec<f32> {
    let mut complex_samples: Vec<Complex<f32>> =
        signal.iter().map(|&v| Complex { re: v, im: 0.0 }).collect();

    fft.process(&mut complex_samples);

    // Convert to magnitudes
    let magnitudes: Vec<f32> = complex_samples.iter().map(|c| c.norm().powf(2.0)).collect();

    magnitudes
}

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

async fn run_bar_visualiser(samples: Arc<Mutex<VecDeque<f32>>>, num_bars: usize) {
    // Rendering parameters
    let bar_width: f32 = (screen_width() - 10.0) / (num_bars as f32);
    let max_height: f32 = screen_height() - 50.0;
    let bar_spacing: f32 = bar_width / 10.0;

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

    let log_ranges = log_ranges(num_bars, SAMPLE_RATE, FFT_SIZE);

    let mut smoothed = vec![0.0_f32; num_bars];

    let rise = 0.5;
    let fall = 0.9;

    // let mut hue = 1.0;
    // let hue_smoothing = 0.05;

    let bar_colour = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

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

        // let mut dominant_freq = 0;
        // let mut max_amplitude = 0.0;

        // for (i, &val) in spectrum.iter().enumerate() {
        //     if val > max_amplitude {
        //         max_amplitude = val;
        //         dominant_freq = i;
        //     }   
        // }

        // let next_hue = dominant_freq as f32 / num_bars as f32;
        // hue = (hue * hue_smoothing) + next_hue * (1.0 - hue_smoothing);
        // let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);

        // bar_colour.r = r;
        // bar_colour.g = g;
        // bar_colour.b = b;
        
        let grouped_spectrum = take_log_max_ranges(&spectrum, &log_ranges);

        for (i, &val) in grouped_spectrum.iter().enumerate() {
            if val > smoothed[i] {
                smoothed[i] = smoothed[i] * rise + val * (1.0 - rise);
            } else {
                smoothed[i] = smoothed[i] * fall + val * (1.0 - fall);
            }
        }

        // Prevent max_val = 0 as that would lead to NaN in smoothing and no bars
        let max_val = smoothed.iter().cloned().fold(1e-6, f32::max);
        let normalised: Vec<f32> = smoothed.iter().map(|m| m / max_val).collect();

        for (i, ampl) in normalised.iter().skip(1).enumerate() {
            let index = i as f32;
            let bar_height = ampl * max_height;
            let x = (index * bar_width) + (index * bar_spacing) + bar_spacing;
            let y = screen_height() - bar_height - 10.0;

            draw_rectangle(x, y, bar_width, bar_height, bar_colour);
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
        Arc::new(Mutex::new(VecDeque::with_capacity(FFT_SIZE)));

    spawn_audio_reader(shared_buffer.clone());

    run_bar_visualiser(shared_buffer.clone(), 32).await;
}
