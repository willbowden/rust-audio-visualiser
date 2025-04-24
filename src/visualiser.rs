use std::f32;

use macroquad::{
    color::Color,
    shapes::draw_rectangle,
    window::{screen_height, screen_width},
};

use crate::{
    bars::{Bars, GroupingStrategy},
    colour::ColourMapper,
    smoothing::SmoothingStrategy,
};

pub struct VisualiserBuilder {
    grouping: GroupingStrategy,
    bars: Bars,
    smoothing: SmoothingStrategy,
    colour: ColourMapper,
}

pub struct Visualiser {
    sampling_rate: usize,
    fft_size: usize,
    grouping: GroupingStrategy,
    bars: Bars,
    smoothing: SmoothingStrategy,
    colour: ColourMapper,
    grouping_ranges: Vec<(usize, usize)>,
    previous_groups: Vec<f32>,
}

impl VisualiserBuilder {
    pub fn new() -> Self {
        Self {
            grouping: GroupingStrategy::LogMax,
            bars: Bars::Normal { num_bars: 24 },
            smoothing: SmoothingStrategy::RiseFall {
                rise: 0.5,
                fall: 0.9,
            },
            colour: ColourMapper::White,
        }
    }

    pub fn with_grouping(mut self, grouping: GroupingStrategy) -> Self {
        self.grouping = grouping;
        self
    }

    pub fn with_bars(mut self, bars: Bars) -> Self {
        self.bars = bars;
        self
    }

    pub fn with_smoothing(mut self, smoothing: SmoothingStrategy) -> Self {
        self.smoothing = smoothing;
        self
    }

    pub fn with_colour_mapper(mut self, colour: ColourMapper) -> Self {
        self.colour = colour;
        self
    }

    pub fn build(self, sampling_rate: usize, fft_size: usize) -> Visualiser {
        let ranges = self
            .grouping
            .create_ranges(self.bars.num_bars(), sampling_rate, fft_size);

        Visualiser {
            sampling_rate,
            fft_size,
            grouping: self.grouping,
            bars: self.bars,
            smoothing: self.smoothing,
            colour: self.colour,
            grouping_ranges: ranges,
            previous_groups: Vec::new(),
        }
    }
}

// TODO: Wire up FFT with Visualiser system and remember to half fft_size for correct spectra
impl Visualiser {
    pub fn update(&self, input: &[f32]) {
        let grouped: Vec<f32> = self.grouping.spectrum_to_bars(input, &self.grouping_ranges);
        let smoothed: Vec<f32> = self.smoothing.smooth(&self.previous_groups, &grouped);
        let colours = self
            .colour
            .calculate_bar_colours(self.bars.num_bars(), &smoothed);

        let max_val = smoothed.iter().cloned().fold(1e-6, f32::max);
        let normalised: Vec<f32> = smoothed.iter().map(|m| m / max_val).collect();

        let bar_width: f32 = (screen_width() - 10.0) / (self.bars.num_bars() as f32);
        let max_height: f32 = screen_height() - 50.0;
        let bar_spacing: f32 = bar_width / 10.0;

        for (i, ampl) in normalised.iter().enumerate() {
            let index = i as f32;
            let bar_height = ampl * max_height;
            let x = (index * bar_width) + (index * bar_spacing) + bar_spacing;
            let y = screen_height() - bar_height - 10.0;

            draw_rectangle(
                x,
                y,
                bar_width,
                bar_height,
                Color {
                    r: colours[i].0,
                    g: colours[i].1,
                    b: colours[i].2,
                    a: colours[i].3,
                },
            );
        }
    }
}
