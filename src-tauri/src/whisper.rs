use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Clone)]
pub struct AudioInputDevice {
    pub name: String,
    pub is_default: bool,
}

/// List all available audio input devices.
pub fn list_input_devices() -> Vec<AudioInputDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut devices = Vec::new();
    if let Ok(input_devices) = host.input_devices() {
        for dev in input_devices {
            if let Ok(name) = dev.name() {
                // Skip devices that don't have any supported input configs
                if dev.supported_input_configs().map(|mut c| c.next().is_some()).unwrap_or(false) {
                    devices.push(AudioInputDevice {
                        is_default: name == default_name,
                        name,
                    });
                }
            }
        }
    }
    devices
}

pub struct RecordingSession {
    _stream: cpal::Stream,
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    /// Current audio RMS level 0..100
    pub level: Arc<AtomicU32>,
}

// cpal::Stream is Send on all major platforms
unsafe impl Send for RecordingSession {}

impl RecordingSession {
    /// Start recording from a specific device name, or the default if empty/None.
    pub fn start(device_name: Option<&str>) -> Result<Self, String> {
        let host = cpal::default_host();

        let device = if let Some(name) = device_name.filter(|n| !n.is_empty()) {
            host.input_devices()
                .map_err(|e| format!("Failed to list devices: {}", e))?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| format!("Device '{}' not found, falling back to default", name))
                .or_else(|e| {
                    eprintln!("{}", e);
                    host.default_input_device()
                        .ok_or_else(|| "No default input device".to_string())
                })?
        } else {
            host.default_input_device()
                .ok_or("No default input device found")?
        };

        eprintln!("Recording from: {}", device.name().unwrap_or_default());

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let level = Arc::new(AtomicU32::new(0));
        let samples_clone = samples.clone();
        let level_clone = level.clone();

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    let mut buf = samples_clone.lock().unwrap();
                    let mut sum_sq: f32 = 0.0;
                    let mut count: usize = 0;
                    for chunk in data.chunks(channels) {
                        let mono = chunk.iter().sum::<f32>() / channels as f32;
                        buf.push(mono);
                        sum_sq += mono * mono;
                        count += 1;
                    }
                    if count > 0 {
                        let rms = (sum_sq / count as f32).sqrt();
                        // Map RMS to 0..100 (RMS of voice is typically 0.01..0.3)
                        let pct = (rms * 300.0).min(100.0) as u32;
                        level_clone.store(pct, Ordering::Relaxed);
                    }
                },
                |err| eprintln!("cpal stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build audio stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        Ok(Self {
            _stream: stream,
            samples,
            sample_rate,
            level,
        })
    }

    pub fn finish(self) -> (Vec<f32>, u32) {
        let samples = self.samples.lock().unwrap().clone();
        (samples, self.sample_rate)
    }
}

/// Encode f32 mono PCM samples as a 16-bit WAV byte buffer.
fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::new(&mut cursor, spec).map_err(|e| format!("WAV init error: {}", e))?;
    for &s in samples {
        writer
            .write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .map_err(|e| format!("WAV write error: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("WAV finalize error: {}", e))?;
    Ok(cursor.into_inner())
}

/// Send WAV bytes to a Whisper-compatible HTTP server and return the transcript.
pub fn transcribe_audio(
    samples: Vec<f32>,
    sample_rate: u32,
    url: &str,
    token: &str,
    model: &str,
) -> Result<String, String> {
    if samples.is_empty() {
        return Ok(String::new());
    }

    let wav_bytes = encode_wav(&samples, sample_rate)?;

    // Build multipart/form-data body manually
    let boundary = "----CopyosityWhisperBoundary7kRx92";
    let mut body: Vec<u8> = Vec::new();

    // --- file field ---
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(&wav_bytes);
    body.extend_from_slice(b"\r\n");

    // --- model field ---
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
    body.extend_from_slice(model.as_bytes());
    body.extend_from_slice(b"\r\n");

    // --- response_format field ---
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
    body.extend_from_slice(b"json");
    body.extend_from_slice(b"\r\n");

    // --- temperature field ---
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"temperature\"\r\n\r\n");
    body.extend_from_slice(b"0");
    body.extend_from_slice(b"\r\n");

    // final boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let mut req = ureq::post(url)
        .set("Content-Type", &content_type)
        .set("Accept", "application/json");

    if !token.is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", token));
    }

    let response = req
        .send_bytes(&body)
        .map_err(|e| format!("Whisper request failed: {}", e))?;

    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| format!("Failed to parse Whisper response: {}", e))?;

    Ok(json["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string())
}
