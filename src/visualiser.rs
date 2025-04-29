use std::f32;

use macroquad::{
    color::WHITE,
    shapes::draw_rectangle,
    window::{screen_height, screen_width},
};

use crate::{
    bars::GroupingStrategy,
    colour::{ColourMapper, StaticColour},
    smoothing::SmoothingStrategy,
};

pub struct VisualiserBuilder {
    grouping: GroupingStrategy,
    smoothing: SmoothingStrategy,
    colour: Box<dyn ColourMapper>,
}

pub struct Visualiser {
    sampling_rate: usize,
    grouping: GroupingStrategy,
    smoothing: SmoothingStrategy,
    colour: Box<dyn ColourMapper>,
    grouping_ranges: Vec<(usize, usize)>,
    // Bars need to be tracked over time to work with smoothing
    bars_to_display: Vec<f32>,
}

impl VisualiserBuilder {
    pub fn new() -> Self {
        Self {
            grouping: GroupingStrategy::LogMax { num_groups: 24 },
            smoothing: SmoothingStrategy::RiseFall {
                rise: 0.5,
                fall: 0.9,
            },
            colour: Box::new(StaticColour::new(WHITE)),
        }
    }

    pub fn with_grouping(mut self, grouping: GroupingStrategy) -> Self {
        self.grouping = grouping;
        self
    }

    pub fn with_smoothing(mut self, smoothing: SmoothingStrategy) -> Self {
        self.smoothing = smoothing;
        self
    }

    pub fn with_colour_mapper(mut self, colour: Box<dyn ColourMapper>) -> Self {
        self.colour = colour;
        self
    }

    pub fn build(self, sampling_rate: usize, fft_size: usize) -> Visualiser {
        let ranges = self.grouping.create_ranges(sampling_rate, fft_size);

        let initial_bars: Vec<f32> = vec![0.0; self.grouping.num_bars()];
        Visualiser {
            sampling_rate,
            grouping: self.grouping,
            smoothing: self.smoothing,
            colour: self.colour,
            grouping_ranges: ranges,
            bars_to_display: initial_bars,
        }
    }
}

impl Visualiser {
    pub fn update(&mut self, input: &[f32]) {
        let grouped: Vec<f32> = self.grouping.group_spectrum(input, &self.grouping_ranges);
        self.smoothing.smooth(&mut self.bars_to_display, &grouped);
        let colour = self.colour.get_colour(input, self.sampling_rate);

        let max_val = self.bars_to_display.iter().cloned().fold(1e-6, f32::max);
        let normalised: Vec<f32> = self.bars_to_display.iter().map(|m| m / max_val).collect();

        let bar_width: f32 = screen_width() / (self.grouping.num_bars() as f32 * 1.1);
        let bar_spacing: f32 = (screen_width() / self.grouping.num_bars() as f32) - bar_width;
        let max_height: f32 = screen_height() - 50.0;

        for (i, ampl) in normalised.iter().enumerate() {
            let index = i as f32;
            let bar_height = ampl * max_height;
            let x = (index * bar_width) + (index * bar_spacing) + bar_spacing;
            let y = screen_height() - bar_height;

            draw_rectangle(x, y, bar_width, bar_height, colour);
        }
    }
}
