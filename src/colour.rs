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
