//! On-device streaming ASR via sherpa-onnx streaming zipformer transducers.
//!
//! Feature-gated behind `local-asr` (see Cargo.toml) so the native sherpa-onnx
//! stack is only pulled when explicitly building/checking it:
//!
//! ```console
//! cargo check --features local-asr
//! ```
//!
//! This is the on-device replacement path for the current hub/whisper HTTP
//! batch transcription: a streaming zipformer transducer emits a growing
//! transcript as audio chunks arrive (true partial results, <300ms class
//! latency on Apple Silicon), with endpointing handled by sherpa's built-in
//! trailing-silence rules. Models are streaming zipformer transducers pulled
//! from Hugging Face on first use (Apache-2.0 stack). RU and EN are separate
//! models — pick per the user's dictation language.
//!
//! Wrapper over the `sherpa-transducers` crate's `OnlineStream`. The next
//! increment feeds live cpal chunks into [`AsrStream::accept`] and surfaces
//! [`AsrStream::transcript`] as interim UI, finalizing on endpoint.

use anyhow::Result;
use sherpa_transducers::asr;

/// English streaming zipformer transducer (320ms chunk). Apache-2.0.
pub const MODEL_EN: &str = "nytopop/zipformer-en-2023-06-21-320ms";
/// Russian streaming zipformer (from the Vosk lineage), packaged for sherpa-onnx.
pub const MODEL_RU: &str = "csukuangfj/sherpa-onnx-streaming-zipformer-small-ru-vosk-2025-08-16";

/// A loaded streaming ASR model. Cheap to open many [`AsrStream`]s from.
pub struct StreamingAsr {
    model: asr::Model,
}

impl StreamingAsr {
    /// Load a streaming model by Hugging Face repo id (downloaded + cached on
    /// first use). Endpointing is enabled so callers can finalize utterances.
    pub async fn load(model_id: &str, num_threads: usize) -> Result<Self> {
        let model = asr::Model::from_pretrained(model_id)
            .await?
            .num_threads(num_threads.max(1))
            .detect_endpoints(true)
            .build()?;
        Ok(Self { model })
    }

    /// The model's native sample rate (feed audio at any rate — it resamples).
    pub fn sample_rate(&self) -> usize {
        self.model.sample_rate()
    }

    /// Open a fresh streaming session.
    pub fn open(&self) -> Result<AsrStream> {
        Ok(AsrStream {
            inner: self.model.online_stream()?,
        })
    }
}

/// One live streaming session: feed chunks with [`accept`](Self::accept), pump
/// [`decode`](Self::decode), read the growing [`transcript`](Self::transcript),
/// and [`reset`](Self::reset) after finalizing an utterance.
pub struct AsrStream {
    inner: asr::OnlineStream,
}

impl AsrStream {
    /// Buffer audio samples (any sample rate — resampled internally).
    pub fn accept(&mut self, sample_rate: usize, samples: &[f32]) {
        self.inner.accept_waveform(sample_rate, samples);
    }

    /// Decode everything buffered so far.
    pub fn decode(&mut self) {
        while self.inner.is_ready() {
            self.inner.decode();
        }
    }

    /// The transcript recognized since the last [`reset`](Self::reset).
    pub fn transcript(&self) -> Result<String> {
        self.inner.result()
    }

    /// Clear recognition state to start a new utterance (call after an endpoint).
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

/// Smoke test: load a model and run a second of silence through the streaming
/// path. Proves the native stack + model download + decode loop work on this
/// machine. Async so the caller supplies the runtime (e.g. `block_on`).
pub async fn selftest(model_id: &str) -> Result<String> {
    let asr = StreamingAsr::load(model_id, 2).await?;
    let mut stream = asr.open()?;
    let sr = asr.sample_rate();
    stream.accept(sr, &vec![0.0f32; sr]); // 1s of silence
    stream.decode();
    stream.transcript()
}
