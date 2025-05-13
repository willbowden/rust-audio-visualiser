mod colour;
mod grouping;
mod smoothing;
mod spectra;
mod visualiser;

use colour::{ChromagramColour, StaticColour};
use spectra::FourierTransform;
use visualiser::VisualiserBuilder;

use macroquad::prelude::*;
use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const SAMPLE_RATE: usize = 44_100;
const FFT_SIZE: usize = 2048;
const FRAME_RATE: usize = 60;

fn get_audio_source() -> Simple {
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: 2,
        rate: SAMPLE_RATE as u32,
    };
    assert!(spec.is_valid());
    // Set lower latency (smaller buffer size)
    let buffer_attr = pulse::def::BufferAttr {
        maxlength: u32::MAX, // Let PulseAudio decide max size
        tlength: u32::MAX,   // Only used for playback
        prebuf: u32::MAX,    // Only used for playback
        minreq: u32::MAX,    // Only used for playback
        fragsize: 1024,      // Lower = lower latency (used for recording)
    };

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    Simple::new(
        None,               // Use the default server
        "AudioVisualiser",  // Our application's name
        Direction::Record,  // We want a recording stream
        Some(source_name),  // Use a monitor source
        "Audio Monitor",    // Description of our stream
        &spec,              // Our sample format
        None,               // Use default channel map
        Some(&buffer_attr), // Use default buffering attributes
    )
    .unwrap()
}

fn spawn_audio_reader(buffer: Arc<Mutex<VecDeque<f32>>>) {
    thread::spawn(move || {
        let mut raw_samples = [0u8; FFT_SIZE * 8]; // 8 bytes per stereo frame (2x f32)

        let s = get_audio_source();

        loop {
            if s.read(&mut raw_samples).is_ok() {
                let mut new_samples = Vec::with_capacity(FFT_SIZE);

                for chunk in raw_samples.chunks_exact(8) {
                    let left = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let right = f32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                    new_samples.push((left + right) / 2.0); // Mono
                }

                let mut buf = buffer.lock().unwrap();
                for s in new_samples {
                    buf.push_back(s);
                }

                // Trim the buffer to stay within the max size
                while buf.len() > FFT_SIZE {
                    buf.pop_front();
                }
            } else {
                eprintln!("Failed to read from audio source");
            }
        }
    });
}

async fn run_bar_visualiser(samples: Arc<Mutex<VecDeque<f32>>>) {
    // Visualiser setup
    let mut visualiser = VisualiserBuilder::new()
        .with_grouping(grouping::GroupingStrategy::LogMax { num_groups: 128 })
        .with_colour_mapper(Box::new(StaticColour::new(WHITE)))
        .build(SAMPLE_RATE, FFT_SIZE, 4);

    // For fixing visualiser FPS
    let mut last_frame_time = 0.0;
    let target_frame_duration = 1.0 / (FRAME_RATE as f64);

    let fft = FourierTransform::new(FFT_SIZE);

    loop {
        let current_time = macroquad::prelude::get_time();
        let frame_time = current_time - last_frame_time;

        clear_background(Color {
            r: 0.1,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        });

        let samples_to_use: Vec<f32> = samples.lock().unwrap().clone().into();

        if samples_to_use.len() < FFT_SIZE {
            next_frame().await;
            continue;
        }

        let spectrum = fft.compute(&samples_to_use);
        visualiser.draw_midi_pitches(&spectrum);
        last_frame_time = current_time;

        if frame_time < target_frame_duration {
            let sleep_duration = (target_frame_duration - frame_time) as u64 * 1_000;
            std::thread::sleep(std::time::Duration::from_millis(sleep_duration));
        }

        next_frame().await
    }
}

#[macroquad::main("Audio Visualiser")]
async fn main() {
    let shared_buffer: Arc<Mutex<VecDeque<f32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(FFT_SIZE)));

    spawn_audio_reader(shared_buffer.clone());

    run_bar_visualiser(shared_buffer.clone()).await;
}
