use macroquad::miniquad::ElapsedQuery;
use macroquad::miniquad::native::egl::EGL_SAMPLES;
use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use std::f32::consts::PI;
use std::os::linux::raw;
use std::thread;

struct ComplexNum {
    real: f32,
    imaginary: f32,
}

#[allow(dead_code)]
fn generate_sinusoid(
    buffer: &mut Vec<f32>,
    frequency: f32,
    sampling_rate: f32,
    amplitude: f32,
    duration: f32,
) {
    let num_samples = (sampling_rate * duration) as usize;
    let dt = 1.0 / sampling_rate;

    for i in 0..num_samples {
        let t = i as f32 * dt;
        buffer.push(amplitude * f32::sin(2.0 * PI * frequency * t));
    }
}

#[allow(dead_code)]
fn dft(k: &f32, data: &Vec<f32>) -> f32 {
    let mut x_k = ComplexNum {
        real: 0.0,
        imaginary: 0.0,
    };
    let big_n = data.len() as f32;

    for (n, val) in data.iter().enumerate() {
        let trig = (2.0 * PI * k * n as f32) / big_n;
        x_k.real += val * trig.cos();
        x_k.imaginary -= val * trig.sin();
    }

    let mag = (x_k.real.powi(2) + x_k.imaginary.powi(2)).sqrt() * 2.0;

    mag / big_n
}

#[allow(dead_code)]
fn spectrum_dft(output: &mut Vec<f32>, data: &Vec<f32>, bins: i32) -> f32 {
    output.clear();
    let mut max = 0.0;
    for i in 0..bins {
        let k = i as f32;
        let result = dft(&k, data);
        if result > max {
            max = result;
        }
        output.push(result);
    }
    max
}

#[allow(dead_code)]
fn monitor_audio() {
    let mut raw_samples: [u8; 1024] = [0; 1024];
    let s = get_audio_source();

    loop {
        s.read(&mut raw_samples).unwrap();
        let mut mono_samples: Vec<f32> = Vec::with_capacity(raw_samples.len() / 2);

        for chunk in raw_samples.chunks_exact(8) {
            let left = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let right = f32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            mono_samples.push((left + right) / 2.0);
        }

        println!("First few samples: {:?}", &mono_samples[..8]);
    }
}

// fn main() {
//     monitor_audio();
// }

#[allow(dead_code)]
fn get_audio_source() -> Simple {
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: 2,
        rate: 44100,
    };
    assert!(spec.is_valid());

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    Simple::new(
        None,              // Use the default server
        "AudioVisualiser", // Our application's name
        Direction::Record, // We want a recording stream
        Some(source_name), // Use a monitor source
        "Audio Monitor",   // Description of our stream
        &spec,             // Our sample format
        None,              // Use default channel map
        None,              // Use default buffering attributes
    )
    .unwrap()
}

async fn run_visualiser() {
    let bins = 256;
    let bar_width: f32 = (screen_width() - 10.0) / (bins as f32);
    let max_height: f32 = screen_height() - 50.0;
    let bar_spacing: f32 = bar_width / 10.0;

    let mut raw_samples: [u8; 1024] = [0; 1024];
    let s = get_audio_source();
    let mut spectrum = Vec::new();

    loop {
        spectrum.clear();
        s.read(&mut raw_samples).unwrap();

        let mut mono_samples: Vec<f32> = Vec::with_capacity(raw_samples.len() / 2);

        
        for chunk in raw_samples.chunks_exact(8) {
            let left = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let right = f32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            mono_samples.push((left + right) / 2.0);
        }
        
        println!("Samples read this frame: {}", mono_samples.len());
        println!("First few samples: {:?}", &mono_samples[..8]);

        let max_amplitude = spectrum_dft(&mut spectrum, &mono_samples, bins);

        clear_background(GRAY);

        for (i, ampl) in spectrum.iter().enumerate() {
            let index = i as f32;
            let intensity: f32 = ampl.clone() as f32;
            let bar_height = (intensity / (max_amplitude as f32)) * max_height;
            let x = (index * bar_width) + (index * bar_spacing) + bar_spacing;
            let y = screen_height() - bar_height - 10.0;

            draw_rectangle(x, y, bar_width, bar_height, WHITE);
        }

        next_frame().await
    }
}

#[macroquad::main("Audio Visualiser")]
async fn main() {
    run_visualiser().await;
}
