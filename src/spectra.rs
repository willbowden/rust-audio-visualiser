use cqt_rs::{CQTParams, Cqt};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::sync::Arc;
use windowfunctions::{Symmetry, WindowFunction, window};

pub fn get_n_largest_indices(items: &[f32], n: usize) -> Vec<usize> {
    let mut values = vec![0.0; n];
    let mut indices: Vec<usize> = vec![items.len(); n];

    for (index, &value) in items.iter().enumerate() {
        for i in 0..n {
            if value > values[i] {
                if i < n - 1 {
                    for j in (i + 1..n).rev() {
                        values[j] = values[j - 1];
                        indices[j] = indices[j - 1];
                    }
                }
                values[i] = value;
                indices[i] = index;
                break;
            }
        }
    }

    indices
}

pub fn chroma_index_to_note(index: usize) -> String {
    match index {
        0 => String::from("C"),
        1 => String::from("C#/Db"),
        2 => String::from("D"),
        3 => String::from("D#/Eb"),
        4 => String::from("E"),
        5 => String::from("F"),
        6 => String::from("F#/Gb"),
        7 => String::from("G"),
        8 => String::from("G#/Ab"),
        9 => String::from("A"),
        10 => String::from("A#/Bb"),
        11 => String::from("B"),
        _ => String::from("UNK"),
    }
}

pub struct FourierTransform {
    fft: Arc<dyn rustfft::Fft<f32>>,
    fft_size: usize,
    window_vec: Vec<f32>,
}

/// Struct that computes Fast Fourier Transforms of size `fft_size`
///
/// Applies a window to signals before processing.
impl FourierTransform {
    pub fn new(fft_size: usize) -> Self {
        // FFT setup
        let mut planner = FftPlanner::<f32>::new();
        let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(fft_size);

        // Hann window to apply pre-FFT
        let window_type = WindowFunction::Hann;
        let symmetry = Symmetry::Symmetric;
        let window_iter = window::<f32>(fft_size, window_type, symmetry);
        let window_vec: Vec<f32> = window_iter.into_iter().collect();
        Self {
            fft,
            fft_size,
            window_vec,
        }
    }

    /// Computes a single FFT on a buffer of real-valued audio samples
    ///
    /// Returns the real half of the FFT spectrum, with length `signal.len() / 2`
    pub fn compute(&self, signal: &[f32]) -> Vec<f32> {
        let mut complex_samples: Vec<Complex<f32>> = signal
            .iter()
            .zip(&self.window_vec)
            .map(|(&value, &w)| Complex {
                re: value * w,
                im: 0.0,
            })
            .collect();

        self.fft.process(&mut complex_samples);

        // Convert to magnitudes
        let magnitudes: Vec<f32> = complex_samples
            .iter()
            .take(complex_samples.len() / 2)
            .map(|c| c.norm().powf(2.0))
            .collect();

        magnitudes
    }
}

/// Takes a frequency-domain spectrum of any length and
///  groups it into a 128-pitch log frequency spectrogram
///
///  Assumes `frequencies` represents 0Hz to (sampling_rate / 2)Hz in uniform intervals
pub fn frequency_to_pitch_spectrum(frequencies: &[f32], sampling_rate: usize) -> [f32; 128] {
    let mut spectrogram = [0.0; 128];
    let freq_per_bin = (sampling_rate as f32 / 2.0) / frequencies.len() as f32;

    let min_pitch: usize = 40; // E2
    let max_pitch: usize = 84; // C6

    for (bin_idx, value) in frequencies.iter().enumerate() {
        let bin_freq = bin_idx as f32 * freq_per_bin;
        let pitch = 69.0 + 12.0 * (bin_freq / 440.0).log2(); // MIDI pitch estimate
        let pitch_idx = pitch.round() as usize;
        // Ignore pitches outside desired range (e.g ignore signals from percussion instruments)
        if pitch_idx < min_pitch || pitch_idx > max_pitch {
            continue;
        }
        if pitch_idx < 128 {
            spectrogram[pitch_idx] += value;
        }
    }

    spectrogram
}

/// Takes a MIDI standard 128-pitch spectrum and collects
///  melodic frequencies into the twelve Western musical notes:
///
/// C, C#, D, D#, E, F, F#, G, G#, A, A#, B
pub fn pitch_spectrum_to_chromagram(pitches: &[f32]) -> [f32; 12] {
    let mut chromagram = [0.0; 12];

    for (p, &val) in pitches.iter().enumerate() {
        chromagram[p % 12] += val;
    }

    chromagram
}

/// Computes the Harmonic Product Spectrum from a uniformly-spaced frequency spectrum
///
/// `downsamples` dictates the number of products used to compute the final result, which
/// will be of length `frequencies.len() / downsamples`
pub fn frequency_to_harmonic_product_spectrum(frequencies: &[f32], downsamples: usize) -> Vec<f32> {
    if downsamples <= 1 {
        return frequencies.to_vec();
    }

    let output_len = frequencies.len() / downsamples;
    let mut result: Vec<f32> = frequencies[0..output_len].to_vec();

    for i in 1..output_len {
        for j in 2..=downsamples {
            result[i] *= frequencies[j * i];
        }
    }

    result
}
