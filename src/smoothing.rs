pub enum SmoothingStrategy {
    RiseFall { rise: f32, fall: f32 },
}

fn rise_fall_smoothing(previous: &[f32], current: &[f32], rise: f32, fall: f32) -> Vec<f32> {
    let mut smoothed = vec![0.0; current.len()];

    for (i, &val) in current.iter().enumerate() {
        if val > previous[i] {
            smoothed[i] = previous[i] * rise + val * (1.0 - rise);
        } else {
            smoothed[i] = previous[i] * fall + val * (1.0 - fall);
        }
    }

    smoothed
}

impl SmoothingStrategy {
    pub fn smooth(&self, previous: &[f32], current: &[f32]) -> Vec<f32> {
        match *self {
            SmoothingStrategy::RiseFall { rise, fall } => {
                rise_fall_smoothing(previous, current, rise, fall)
            }
        }
    }
}
