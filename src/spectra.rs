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

/// Takes a MIDI standard pitch spectrum and collects
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

    let max_index = frequencies.len() / downsamples;
    let mut result = frequencies.to_vec();

    for i in 0..max_index {
        for j in 1..=downsamples {
            result[i] = frequencies[j * i] * frequencies[i];
        }
    }

    result
}
