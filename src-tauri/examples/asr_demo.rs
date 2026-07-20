//! Live streaming-ASR demo for Copyosity's on-device transcription (sherpa-onnx).
//!
//!   cargo run --example asr_demo --features local-asr             # mic, live
//!   cargo run --example asr_demo --features local-asr -- clip.wav # from a WAV
//!   cargo run --example asr_demo --features local-asr -- --ru     # Russian model
//!
//! Prints the growing transcript in place as audio streams in (partial results),
//! exactly the signal Copyosity's overlay will render. First run downloads the
//! model from Hugging Face. Needs a static/prebuilt sherpa-onnx (see Cargo.toml).

use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use copyosity_lib::local_asr::{StreamingAsr, MODEL_EN, MODEL_RU};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ru = args.iter().any(|a| a == "--ru");
    let wav_path = args.iter().find(|a| a.ends_with(".wav")).cloned();
    let model = if ru { MODEL_RU } else { MODEL_EN };

    eprintln!("[asr_demo] loading {model} (first run downloads from Hugging Face)…");
    let asr = StreamingAsr::load(model, 2).await?;
    let mut stream = asr.open()?;
    eprintln!("[asr_demo] model ready (native rate {} Hz)", asr.sample_rate());

    match wav_path {
        Some(path) => run_wav(&mut stream, &path),
        None => run_mic(&mut stream),
    }
}

/// Stream a WAV file through the recognizer in ~100ms chunks (verifiable path).
fn run_wav(stream: &mut copyosity_lib::local_asr::AsrStream, path: &str) -> Result<()> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let ch = spec.channels as usize;

    // Decode to f32 and downmix to mono.
    let mono: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max)
                .collect()
        }
    };
    let mono: Vec<f32> = if ch > 1 {
        mono.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()
    } else {
        mono
    };

    eprintln!(
        "[asr_demo] streaming {:.1}s of audio from {path}…\n",
        mono.len() as f32 / spec.sample_rate as f32
    );
    let chunk = (spec.sample_rate as usize / 10).max(1); // ~100ms
    for c in mono.chunks(chunk) {
        stream.accept(spec.sample_rate as usize, c);
        stream.decode();
        print!("\r\x1b[K{}", stream.transcript()?);
        std::io::stdout().flush().ok();
    }
    // Trailing silence flushes the final word(s) out of the transducer.
    stream.accept(spec.sample_rate as usize, &vec![0.0f32; spec.sample_rate as usize / 2]);
    stream.decode();
    println!("\n\n[asr_demo] FINAL: {}", stream.transcript()?);
    Ok(())
}

/// Capture the default mic and print the transcript live until Ctrl+C.
fn run_mic(stream: &mut copyosity_lib::local_asr::AsrStream) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("no default input device"))?;
    let supported = device.default_input_config()?;
    let sr = supported.sample_rate().0 as usize;
    let ch = supported.channels() as usize;
    let cfg: cpal::StreamConfig = supported.clone().into();
    eprintln!("[asr_demo] mic: {} @ {sr} Hz, {ch}ch — speak! (Ctrl+C to stop)\n", device.name().unwrap_or_default());

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let err_fn = |e| eprintln!("[asr_demo] stream error: {e}");
    let cpal_stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &cfg,
            move |data: &[f32], _: &_| {
                let mono: Vec<f32> = data.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
                tx.send(mono).ok();
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &cfg,
            move |data: &[i16], _: &_| {
                let mono: Vec<f32> = data
                    .chunks(ch)
                    .map(|f| f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / ch as f32)
                    .collect();
                tx.send(mono).ok();
            },
            err_fn,
            None,
        )?,
        other => anyhow::bail!("unsupported sample format: {other:?}"),
    };
    cpal_stream.play()?;

    loop {
        let mut got = false;
        while let Ok(buf) = rx.try_recv() {
            stream.accept(sr, &buf);
            got = true;
        }
        if got {
            stream.decode();
            print!("\r\x1b[K{}", stream.transcript()?);
            std::io::stdout().flush().ok();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
