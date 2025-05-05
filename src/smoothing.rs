pub enum SmoothingStrategy {
    RiseFall { rise: f32, fall: f32 },
    None,
}

fn rise_fall_smoothing(previous: &mut [f32], current: &[f32], rise: f32, fall: f32) {
    // TODO: Fix this
    if previous.is_empty() {
        previous.copy_from_slice(current);
        return;
    }
    for (i, &val) in current.iter().enumerate() {
        if val > previous[i] {
            previous[i] = previous[i] * rise + val * (1.0 - rise);
        } else {
            previous[i] = previous[i] * fall + val * (1.0 - fall);
        }
    }
}

impl SmoothingStrategy {
    // Apply smoothing strategy inplace
    pub fn smooth(&self, previous: &mut [f32], current: &[f32]) {
        match *self {
            SmoothingStrategy::RiseFall { rise, fall } => {
                rise_fall_smoothing(previous, current, rise, fall)
            }
            SmoothingStrategy::None => (),
        }
    }
}
