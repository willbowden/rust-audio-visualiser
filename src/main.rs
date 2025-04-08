// use psimple::Simple;
// use pulse::sample::{Format, Spec};
// use pulse::stream::Direction;
use std::f64::consts::PI;

struct ComplexNum {
    real: f64,
    imaginary: f64,
}

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

fn test_dft() {
    let frequency = 4.0;
    let sampling_rate = 16.0;
    let bins = 8;
    let amplitude = 1.0;
    let duration = 1.0;

    let num_samples = (sampling_rate * duration) as usize;
    let dt = 1.0 / sampling_rate;

    let mut data: Vec<f64> = Vec::new();
    for i in 0..num_samples {
        let t = i as f64 * dt;
        data.push(amplitude * f64::sin(2.0 * PI * frequency * t));
    }

    println!("Samples:\n{:?}", &data);

    let mut out: Vec<f64> = Vec::new();

    for i in 0..bins {
        let k = i as f64;
        out.push(dft(&k, &data));
    }

    println!("Result:\n{:?}", out);
}

// fn monitor_audio() {
//     let spec = Spec {
//         format: Format::S16NE,
//         channels: 2,
//         rate: 44100,
//     };
//     assert!(spec.is_valid());

//     let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

//     let s = Simple::new(
//         None,              // Use the default server
//         "AudioVisualiser", // Our application's name
//         Direction::Record, // We want a recording stream
//         Some(source_name), // Use a monitor source
//         "Audio Monitor",   // Description of our stream
//         &spec,             // Our sample format
//         None,              // Use default channel map
//         None,              // Use default buffering attributes
//     )
//     .unwrap();

//     let mut samples: [u8; 1024] = [0; 1024];

//     loop {
//         s.read(&mut samples).unwrap();
//         for val in samples {
//             println!("{}", val);
//         }
//     }
// }

fn main() {
    test_dft();
}
