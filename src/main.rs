use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use std::f64::consts::PI;

struct ComplexNum {
    real: f64,
    imaginary: f64,
}

#[allow(dead_code)]
fn generate_sinusoid(
    buffer: &mut Vec<f64>,
    frequency: f64,
    sampling_rate: f64,
    amplitude: f64,
    duration: f64,
) {
    let num_samples = (sampling_rate * duration) as usize;
    let dt = 1.0 / sampling_rate;

    for i in 0..num_samples {
        let t = i as f64 * dt;
        buffer.push(amplitude * f64::sin(2.0 * PI * frequency * t));
    }
}

#[allow(dead_code)]
fn dft(k: &f64, data: &Vec<f64>) -> f64 {
    let mut x_k = ComplexNum {
        real: 0.0,
        imaginary: 0.0,
    };
    let big_n = data.len() as f64;

    for (n, val) in data.iter().enumerate() {
        let trig = (2.0 * PI * k * n as f64) / big_n;
        x_k.real += val * trig.cos();
        x_k.imaginary += val * trig.sin();
    }

    let mag = (x_k.real.powi(2) + x_k.imaginary.powi(2)).sqrt() * 2.0;

    mag / big_n
}

#[allow(dead_code)]
fn spectrum_dft(output: &mut Vec<f64>, data: &Vec<f64>, bins: i32) {
    for i in 0..bins {
        let k = i as f64;
        output.push(dft(&k, &data));
    }
}

#[allow(dead_code)]
fn monitor_audio() {
    let spec = Spec {
        format: Format::S16NE,
        channels: 2,
        rate: 44100,
    };
    assert!(spec.is_valid());

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    let s = Simple::new(
        None,              // Use the default server
        "AudioVisualiser", // Our application's name
        Direction::Record, // We want a recording stream
        Some(source_name), // Use a monitor source
        "Audio Monitor",   // Description of our stream
        &spec,             // Our sample format
        None,              // Use default channel map
        None,              // Use default buffering attributes
    )
    .unwrap();

    let mut samples: [u8; 1024] = [0; 1024];

    loop {
        s.read(&mut samples).unwrap();
        for val in samples {
            println!("{}", val);
        }
    }
}

// fn main() {
//     monitor_audio();
// }

#[macroquad::main("Audio Visualiser")]
async fn main() {
    let spec = Spec {
        format: Format::S16NE,
        channels: 2,
        rate: 44100,
    };
    assert!(spec.is_valid());

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    let s = Simple::new(
        None,              // Use the default server
        "AudioVisualiser", // Our application's name
        Direction::Record, // We want a recording stream
        Some(source_name), // Use a monitor source
        "Audio Monitor",   // Description of our stream
        &spec,             // Our sample format
        None,              // Use default channel map
        None,              // Use default buffering attributes
    )
    .unwrap();

    let bins = 512;
    let bar_width: f32 = (screen_width() - 10.0) / (bins as f32);
    let max_height: f32 = screen_height() - 20.0;
    let bar_spacing: f32 = bar_width / 2.0;

    let mut samples: [u8; 1024] = [0; 1024];
    let mut spectrum = Vec::new();

    loop {
        clear_background(GRAY);

        s.read(&mut samples).unwrap();

        // for val in samples {
        //     println!("{}", val);
        // }

        spectrum_dft(
            &mut spectrum,
            &samples.iter().map(|&e| e as f64).collect(),
            bins,
        );

        // println!("Drawing spectrum:\n{:?}", spectrum);

        for (i, ampl) in spectrum.iter().enumerate() {
            let index = i as f32;
            let intensity: f32 = ampl.clone() as f32;
            let bar_height = max_height * intensity;
            let x = (index * bar_width) + (index * bar_spacing) + bar_spacing;
            let y = 10.0;

            draw_rectangle(x, y, bar_width, bar_height, WHITE);
        }

        next_frame().await
    }
}
