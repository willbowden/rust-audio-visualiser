use std::sync::{Arc, Mutex};

use cpal::{StreamConfig, SupportedOutputConfigs};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let host = cpal::default_host();

    let device_name = "pulse";
    let device = host
        .output_devices()
        .unwrap()
        .find(|d| d.name().unwrap() == device_name)
        .expect("Failed to find specified output device");

    let mut supported_configs = device.supported_output_configs().unwrap();

    let config: StreamConfig = supported_configs.next().into();

    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);

    let sample = Arc::new(Mutex::new(0.0f32));

    let write_tone = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut sample = sample.lock().unwrap();
        let frequency = 80.0;
        let sample_rate = 44100.0;
        let amplitude = 0.2;
        for i in 0..data.len() {
            let phase = (i as f32 / sample_rate) * frequency * 2.0 * std::f32::consts::PI;
            *sample = amplitude * phase.sin();
            data[i] = *sample;
        }
    };

    let stream = device
        .build_output_stream(&config, write_tone, err_fn, None)
        .unwrap();

    stream.play().unwrap();

    std::thread::park();
}
