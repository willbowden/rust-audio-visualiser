use std::cmp::max;

pub enum ColourMapper {
    White,
}

impl ColourMapper {
    pub fn calculate_bar_colours(
        &self,
        num_bars: usize,
        bars: &[f32],
    ) -> Vec<(f32, f32, f32, f32)> {
        match *self {
            ColourMapper::White => vec![(1.0, 1.0, 1.0, 1.0); num_bars],
        }
    }
}

/// Takes a frequency-domain spectrum of any length and
///  groups it into a 128-pitch log frequency spectrogram
pub fn fourier_to_pitch_spectrum(frequencies: &[f32], sampling_rate: usize) -> [f32; 128] {
    let mut spectrogram = [0.0; 128];
    let freq_per_bin = sampling_rate as f32 / frequencies.len() as f32;
    let mut prev_index: usize = 0;

    for (p, val) in spectrogram.iter_mut().enumerate() {
        let f_pitch: f32 = 2.0_f32.powf((p as f32 - 69.0) / 12.0) * 440.0;

        let next_index: usize = (f_pitch / freq_per_bin).round() as usize;
        let next_index = max(prev_index + 1, next_index); // Ensure at least 1 bin

        *val = frequencies[prev_index..next_index].iter().sum();
        prev_index = next_index;
    }

    spectrogram
}

/// Takes a 128-pitch log frequency spectrogram and collects
///  melodic frequencies into the twelve Western musical notes:
///
/// C, C#, D, D#, E, F, F#, G, G#, A, A#, B
///
/// Returns the *normalised* intensities for each note
pub fn pitch_spectrum_to_chromagram(pitches: &[f32; 128]) -> [f32; 12] {
    let mut chromagram = [0.0; 12];

    for (p, &val) in pitches.iter().enumerate() {
        chromagram[p % 12] += val;
    }

    let sum: f32 = chromagram.iter().sum();
    if sum > 0.0 {
        chromagram.iter_mut().for_each(|val| *val /= sum);
    }

    chromagram
}
