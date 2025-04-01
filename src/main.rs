use psimple::Simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;

fn main() {
    let spec = Spec {
        format: Format::S16NE,
        channels: 2,
        rate: 44100,
    };
    assert!(spec.is_valid());

    let source_name = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";

    let s = Simple::new(
        None,                // Use the default server
        "FooApp",            // Our application's name
        Direction::Record,   // We want a recording stream - CHANGED FROM Playback
        Some(source_name),   // Use a monitor source - CHANGED FROM None
        "Audio Monitor",     // Description of our stream
        &spec,               // Our sample format
        None,                // Use default channel map
        None,                // Use default buffering attributes
    )
    .unwrap();


    let mut samples: [u8;1024] = [0; 1024];

    loop {
        s.read(&mut samples).unwrap();
        for val in samples {
            println!("{}", val);
        }
    }

}
