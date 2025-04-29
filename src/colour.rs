use std::{
    cmp::max,
    collections::{VecDeque, vec_deque},
};

use macroquad::color::{Color, WHITE};

use crate::spectra::{frequency_to_pitch_spectrum, pitch_spectrum_to_chromagram};

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
    hue_vector: (f32, f32),
    smoothing_factor: f32,
    smoothed_chromagram: [f32; 12],
}

impl ChromagramColour {
    pub fn new(smoothing_factor: f32) -> Self {
        Self {
            hue_vector: (0.0, 0.0),
            smoothing_factor,
            smoothed_chromagram: [0.0; 12],
        }
    }
}

impl ColourMapper for ChromagramColour {
    fn get_colour(&mut self, spectrum: &[f32], sampling_rate: usize) -> Color {
        let chromagram =
            pitch_spectrum_to_chromagram(&frequency_to_pitch_spectrum(spectrum, sampling_rate));

        for (i, &value) in chromagram.iter().enumerate() {
            self.smoothed_chromagram[i] = (1.0 - self.smoothing_factor) * value
                + self.smoothing_factor * self.smoothed_chromagram[i];
        }

        let mut hue_vector: (f32, f32) = (0.0_f32.cos(), 0.0_f32.sin());

        for (i, &intensity) in self.smoothed_chromagram.iter().enumerate() {
            let hue: f32 = (i as f32 * 30.0).to_radians();
            // Add weighted hue vectors together
            hue_vector.0 += intensity * hue.cos();
            hue_vector.1 += intensity * hue.sin();
        }

        self.hue_vector.0 = (1.0 - self.smoothing_factor) * hue_vector.0
            + self.smoothing_factor * self.hue_vector.0;
        self.hue_vector.1 = (1.0 - self.smoothing_factor) * hue_vector.1
            + self.smoothing_factor * self.hue_vector.1;

        // theta = atan2(y, x)
        let final_hue = f32::atan2(self.hue_vector.1, self.hue_vector.0).to_degrees();
        let final_colour = hsv_to_rgb(final_hue, 1.0, 1.0);

        Color {
            r: final_colour.0,
            g: final_colour.1,
            b: final_colour.2,
            a: 1.0,
        }
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
