use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use windowfunctions::{Symmetry, WindowFunction, window};

use core::num;
use std::cmp::max;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const SAMPLE_RATE: usize = 44_100;
const FFT_SIZE: usize = 2048;
const BAR_COUNT: usize = 24;

/// Computes a frequency spectrum using FFT, optionally across multiple windows
///
/// Requires `window_function`: a precomputed (e.g Hamming) window signal of length `window_size`
///
/// Returns a vector of length `window_size / 2` thanks to Nyquist's limit
// fn windowed_fft(
//     signal: &[f32],
//     fft: &Arc<dyn rustfft::Fft<f32>>,
//     window_function: &Vec<f32>,
//     window_size: usize,
//     num_windows: usize,
//     overlap: f32,
// ) -> Vec<f32> {
//     let samples_required;
//     let buffer_length = signal.len();

//     if overlap == 0.0 {
//         samples_required = window_size * num_windows;
//     } else {
//         samples_required = window_size + (((num_windows - 1) * window_size) as f32 * overlap) as usize;
//     }

//     if window_function.len() != window_size {
//         panic!("Cannot apply windowing function of length {} to windows of length {}!", window_function.len(), window_size);
//     }

//     if buffer_length < samples_required as usize {
//         panic!(
//             "Attempting to run {} FFT windows of size {} with {}% overlap on buffer of length {}.\nBuffer must be {} long!",
//             num_windows,
//             window_size,
//             (overlap*100.0) as usize,
//             buffer_length,
//             samples_required
//         );
//     }

//     // Calculate indices into buffer for windows
//     let step = (window_size as f32 * overlap) as usize;
//     let mut first_index = buffer_length - window_size;

//     // Allow for 1 window where `window_size` == `buffer_length`
//     if first_index > 0 {
//         first_index -= 1;
//     }

//     let mut averaged_windows: Vec<f32> = vec![0.0; window_size];

//     // Compute FFT for each of our windows, moving back to front on the buffer
//     for i in 0..num_windows {
//         let start = first_index - (step * i);
//         let end = start + window_size;
//         let window_slice = &signal[start..end];

//         // Build windowed complex buffer
//         let mut buffer: Vec<Complex<f32>> = window_slice
//             .iter()
//             .zip(window_function.iter())
//             .map(|(&x, &w)| Complex { re: x * w, im: 0.0 })
//             .collect();

//         let amplitudes = compute_fft(&mut buffer, fft);
//         for (i, &val) in amplitudes.iter().enumerate() {
//             averaged_windows[i] += val as f32;
//         }
//     }

//     for val in &mut averaged_windows {
//         *val /= num_windows as f32;
//     }

//     averaged_windows
// }

/// Compute how to split an FFT of length `fft_size` into `num_bins` logarithmically spaced bins
///
/// To be computed in advance and reused across FFT processes
fn compute_log_ranges(num_bins: usize, sample_rate: usize, fft_size: usize) -> Vec<(usize, usize)> {
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

    for (i, &(start, stop)) in ranges.iter().enumerate() {
        println!("Frequency bin {}: {}Hz-{}Hz", i+1, ((start as f32) * freq_per_bin).round(), ((stop as f32) * freq_per_bin).round());
    }

    ranges
}

/// Converts an FFT spectrum into `num_bars` bars spaced logarithmically
/// in the frequency domain
fn to_logarithmic_bars(spectrum: &Vec<f32>, bar_ranges: &Vec<(usize, usize)>) -> Vec<f32> {
    let mut log_bars = vec![0.0; bar_ranges.len()];

    for (i, &(start, end)) in bar_ranges.iter().enumerate() {
        let slice: &[f32] = &spectrum[start..end];
        let sum: f32 = slice.iter().sum();
        log_bars[i] = ((sum / slice.len() as f32) + 1.0).log10();
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
                while buf.len() > FFT_SIZE {
                    buf.pop_front();
                }
            } else {
                eprintln!("Failed to read from audio source");
            }
        }
    });
}

/// Computes a single FFT on a buffer of complex samples
///
/// Note that only the first half of the values will be real frequencies
fn compute_fft(signal: &Vec<f32>, fft: &Arc<dyn rustfft::Fft<f32>>) -> Vec<f32> {
    let mut complex_samples: Vec<Complex<f32>> =
        signal.iter().map(|&v| Complex { re: v, im: 0.0 }).collect();

    fft.process(&mut complex_samples);

    // Convert to magnitudes
    let magnitudes: Vec<f32> = complex_samples.iter().map(|c| c.norm()).collect();

    magnitudes
}

async fn run_bar_visualiser(samples: Arc<Mutex<VecDeque<f32>>>) {
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

    let log_ranges = compute_log_ranges(BAR_COUNT, SAMPLE_RATE, FFT_SIZE);

    loop {
        let current_time = macroquad::prelude::get_time();
        let frame_time = current_time - last_frame_time;

        clear_background(GRAY);

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

        // let log_bars: Vec<f32> = bin_ranges
        //     .iter()
        //     .map(|(start, end)| {
        //         if *end > spectrum.len() {
        //             return 0.0;
        //         }
        //         let slice = &spectrum[*start..*end];
        //         slice.iter().copied().fold(0.0, f32::max)
        //     })
        //     .collect();

        let spectrum_log = to_logarithmic_bars(&spectrum, &log_ranges);

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
        Arc::new(Mutex::new(VecDeque::with_capacity(FFT_SIZE)));

    spawn_audio_reader(shared_buffer.clone());

    run_bar_visualiser(shared_buffer.clone()).await;
}
