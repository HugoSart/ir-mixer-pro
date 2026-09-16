use ir_core::{AudioBuffer, SampleRate};
use ir_dsp::{DspEngine, EngineConfig, PreparedSlot, SlotParameters};
use std::time::Instant;

fn main() {
    let config = EngineConfig {
        sample_rate: 48_000,
        block_size: 64,
        max_active_irs: 16,
        ..EngineConfig::default()
    };
    let ir = AudioBuffer::mono(
        SampleRate(48_000),
        (0..2048)
            .map(|index| {
                if index == 0 {
                    1.0
                } else {
                    (-(index as f32) / 280.0).exp() * (index as f32 * 0.17).sin() * 0.1
                }
            })
            .collect(),
    );
    let mut engine = DspEngine::new(config);
    for id in 0..16 {
        engine
            .add_slot(PreparedSlot::new(id, &ir, config, SlotParameters::default()).unwrap())
            .unwrap();
    }
    let input = vec![0.1; config.block_size];
    let mut left = vec![0.0; config.block_size];
    let mut right = vec![0.0; config.block_size];
    for _ in 0..100 {
        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();
    }
    let iterations = 10_000;
    let started = Instant::now();
    for _ in 0..iterations {
        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();
    }
    let elapsed = started.elapsed();
    let callback = elapsed.as_secs_f64() / iterations as f64;
    let deadline = config.block_size as f64 / config.sample_rate as f64;
    println!(
        "16 IRs, 2048 samples, 48 kHz / 64: {:.3} µs per callback ({:.1}% of deadline)",
        callback * 1_000_000.0,
        callback / deadline * 100.0
    );
    if callback > deadline * 0.5 {
        eprintln!("WARNING: benchmark exceeds the 50% plugin gate");
        std::process::exit(2);
    }
}
