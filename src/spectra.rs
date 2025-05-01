use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::sync::Arc;
use windowfunctions::{Symmetry, WindowFunction, window};

pub struct FourierTransform {
    fft: Arc<dyn rustfft::Fft<f32>>,
    fft_size: usize,
    window_vec: Vec<f32>,
}

/// Struct that computes Fast Fourier Transforms of size `fft_size`
///
/// Applies a Hamming Window to singals before processing.
impl FourierTransform {
    pub fn new(fft_size: usize) -> Self {
        // FFT setup
        let mut planner = FftPlanner::<f32>::new();
        let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(fft_size);

        // Hamming window to apply pre-FFT
        let window_type = WindowFunction::Hamming;
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
    let min_pitch: usize = 32; // E2, roughly 82Hz
    let max_pitch: usize = 84; // C6 ~1kHz

    let mut spectrogram = [0.0; 128];
    let freq_per_bin = (sampling_rate as f32 / 2.0) / frequencies.len() as f32;
    let mut prev_index: usize = 0;

    for (p, val) in spectrogram.iter_mut().enumerate() {
        let f_pitch: f32 = 2.0_f32.powf((p as f32 - 69.0) / 12.0) * 440.0;

        if p < min_pitch || p > max_pitch {
            *val = 0.0;
        }

        let next_index: usize = (f_pitch / freq_per_bin).floor() as usize;

        *val = frequencies[prev_index..next_index].iter().sum();
        prev_index = next_index;
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
    if downsamples == 0 {
        return frequencies.to_vec();
    }

    let output_len = frequencies.len() / downsamples;
    let mut result: Vec<f32> = frequencies[0..output_len].to_vec();

    for i in 0..output_len {
        for j in 2..=downsamples {
            result[i] *= frequencies[j * i];
        }
    }

    result
}
