use std::cmp::max;

/// Compute how to split an FFT of length `fft_size` into `num_bins` using common music frequency ranges
///
/// To be computed in advance and reused across FFT processes
fn log_ranges(num_bars: usize, sample_rate: usize, fft_size: usize) -> Vec<(usize, usize)> {
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

    let mut bins_per_range = weights.map(|(_, v)| (num_bars as f32 * v).floor() as usize);

    let mut bin_sum: usize = bins_per_range.iter().sum();
    let mut index = 0;

    while bin_sum < num_bars {
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
fn take_log_mean_ranges(spectrum: &[f32], bar_ranges: &[(usize, usize)]) -> Vec<f32> {
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
fn take_log_max_ranges(spectrum: &[f32], bar_ranges: &[(usize, usize)]) -> Vec<f32> {
    let mut log_bars = vec![0.0; bar_ranges.len()];

    for (i, &(start, end)) in bar_ranges.iter().enumerate() {
        let slice: &[f32] = &spectrum[start..end];
        let max_value: f32 = slice.iter().copied().fold(0.0, f32::max);
        log_bars[i] = (max_value + 1.0).log2();
    }

    log_bars
}

pub enum GroupingStrategy {
    NoGrouping,
    LogMax,
    LogMean,
    GammaCorrected { gamma: f32 },
}

impl GroupingStrategy {
    pub fn create_ranges(
        &self,
        num_bars: usize,
        sample_rate: usize,
        fft_size: usize,
    ) -> Vec<(usize, usize)> {
        match self {
            GroupingStrategy::NoGrouping => Vec::new(),
            GroupingStrategy::LogMax => log_ranges(num_bars, sample_rate, fft_size),
            GroupingStrategy::LogMean => log_ranges(num_bars, sample_rate, fft_size),
            GroupingStrategy::GammaCorrected { gamma } => {
                gamma_corrected_ranges(num_bars, sample_rate, fft_size, *gamma)
            }
        }
    }

    pub fn spectrum_to_bars(&self, spectrum: &[f32], bar_ranges: &[(usize, usize)]) -> Vec<f32> {
        match *self {
            GroupingStrategy::NoGrouping => spectrum.to_vec(),
            GroupingStrategy::LogMax => take_log_max_ranges(spectrum, bar_ranges),
            GroupingStrategy::LogMean => take_log_mean_ranges(spectrum, bar_ranges),
            GroupingStrategy::GammaCorrected { gamma: _ } => {
                take_log_mean_ranges(spectrum, bar_ranges)
            }
        }
    }
}

// TODO: How do we handle num_bars when NoGrouping is used? That would mean num_bars = fft_size /
// 2.0,
pub enum Bars {
    Normal { num_bars: usize },
    LeftMirrored { num_bars: usize },
    RightMirrored { num_bars: usize },
}

impl Bars {
    pub fn num_bars(&self) -> usize {
        match self {
            Bars::Normal { num_bars }
            | Bars::LeftMirrored { num_bars }
            | Bars::RightMirrored { num_bars } => *num_bars,
        }
    }
}
