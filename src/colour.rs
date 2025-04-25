use std::{
    cmp::max,
    collections::{VecDeque, vec_deque},
};

use macroquad::color::{Color, WHITE};

pub trait ColourMapper {
    fn get_colour(&mut self, spectrum: &[f32], sampling_rate: usize) -> Color;
}

pub struct StaticColour {
    colour: Color,
}

impl ColourMapper for StaticColour {
    fn get_colour(&mut self, spectrum: &[f32], sampling_rate: usize) -> Color {
        self.colour
    }
}

impl StaticColour {
    pub fn new(colour: Color) -> Self {
        Self { colour }
    }
}

pub struct ChromagramColour {
    colour: Color,
    smoothing_factor: f32,
    smoothed_chromagram: [f32; 12],
}

impl ChromagramColour {
    pub fn new(smoothing_factor: f32) -> Self {
        Self {
            colour: WHITE,
            smoothing_factor,
            smoothed_chromagram: [0.0; 12],
        }
    }
}

// TODO: Switch to hue vector averaging
impl ColourMapper for ChromagramColour {
    fn get_colour(&mut self, spectrum: &[f32], sampling_rate: usize) -> Color {
        let chromagram =
            pitch_spectrum_to_chromagram(&fourier_to_pitch_spectrum(spectrum, sampling_rate));

        for (i, &value) in chromagram.iter().enumerate() {
            self.smoothed_chromagram[i] = self.smoothing_factor * value
                + (1.0 - self.smoothing_factor) * self.smoothed_chromagram[i];
        }

        let mut final_colour: (f32, f32, f32) = (0.0, 0.0, 0.0);

        for (i, &intensity) in self.smoothed_chromagram.iter().enumerate() {
            let hue: f32 = i as f32 * 30.0;
            let colour = hsv_to_rgb(hue, 1.0, 1.0);
            final_colour.0 += colour.0 * intensity;
            final_colour.1 += colour.1 * intensity;
            final_colour.2 += colour.2 * intensity;
        }

        self.colour = Color {
            r: final_colour.0,
            g: final_colour.1,
            b: final_colour.2,
            a: 1.0,
        };

        self.colour
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = h.rem_euclid(360.0) / 60.0; // hue sector
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    (r1 + m, g1 + m, b1 + m)
}

/// Takes a frequency-domain spectrum of any length and
///  groups it into a 128-pitch log frequency spectrogram
fn fourier_to_pitch_spectrum(frequencies: &[f32], sampling_rate: usize) -> [f32; 128] {
    let mut spectrogram = [0.0; 128];
    let freq_per_bin = sampling_rate as f32 / frequencies.len() as f32;
    let mut prev_index: usize = 0;

    for (p, val) in spectrogram.iter_mut().enumerate() {
        let f_pitch: f32 = 2.0_f32.powf((p as f32 - 69.0) / 12.0) * 440.0;

        let next_index: usize = (f_pitch / freq_per_bin).floor() as usize;

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
fn pitch_spectrum_to_chromagram(pitches: &[f32; 128]) -> [f32; 12] {
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
