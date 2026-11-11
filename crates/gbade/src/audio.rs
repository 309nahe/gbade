//! Sound output with cpal (Core Audio on macOS).
//!
//! The emulator produces stereo 16-bit samples every frame; they go into a
//! small queue that the audio callback drains. If the queue runs dry we play
//! silence; if it grows too big (e.g. the game ran fast) we drop old samples
//! to keep latency low.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioOut {
    _stream: Option<cpal::Stream>,
    queue: Arc<Mutex<VecDeque<i16>>>,
    volume: Arc<AtomicU32>,
    pub sample_rate: u32,
    pub error: Option<String>,
}

impl AudioOut {
    /// Open the default output device. Never fails: without a device you
    /// simply get no sound (and `error` says why).
    pub fn new() -> AudioOut {
        let queue = Arc::new(Mutex::new(VecDeque::with_capacity(48000)));
        let volume = Arc::new(AtomicU32::new(0.8f32.to_bits()));
        match Self::open(queue.clone(), volume.clone()) {
            Ok((stream, rate)) => AudioOut { _stream: Some(stream), queue, volume, sample_rate: rate, error: None },
            Err(e) => AudioOut { _stream: None, queue, volume, sample_rate: 48000, error: Some(e) },
        }
    }

    fn open(queue: Arc<Mutex<VecDeque<i16>>>, volume: Arc<AtomicU32>) -> Result<(cpal::Stream, u32), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let channels = config.channels as usize;
        let rate = config.sample_rate;
        let err_fn = |e| eprintln!("audio error: {e}");
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _| fill(data, channels, &queue, &volume, |s| s as f32 / 32768.0),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                config,
                move |data: &mut [i16], _| fill(data, channels, &queue, &volume, |s| s),
                err_fn,
                None,
            ),
            other => return Err(format!("unsupported audio sample format {other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((stream, rate))
    }

    /// Queue interleaved stereo samples.
    pub fn push(&self, samples: &[i16]) {
        if self._stream.is_none() {
            return;
        }
        let mut q = self.queue.lock().unwrap();
        q.extend(samples.iter().copied());
        // Keep at most ~120 ms queued.
        let max = (self.sample_rate as usize * 2) * 120 / 1000;
        while q.len() > max {
            q.pop_front();
        }
    }

    pub fn clear(&self) {
        self.queue.lock().unwrap().clear();
    }

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

fn fill<T: Copy + Default>(
    data: &mut [T],
    channels: usize,
    queue: &Mutex<VecDeque<i16>>,
    volume: &AtomicU32,
    conv: impl Fn(i16) -> T,
) {
    let vol = f32::from_bits(volume.load(Ordering::Relaxed));
    let mut q = queue.lock().unwrap();
    for frame in data.chunks_mut(channels.max(1)) {
        let (l, r) = if q.len() >= 2 {
            (q.pop_front().unwrap(), q.pop_front().unwrap())
        } else {
            (0, 0)
        };
        let l = (l as f32 * vol) as i16;
        let r = (r as f32 * vol) as i16;
        for (i, s) in frame.iter_mut().enumerate() {
            *s = conv(if i % 2 == 0 { l } else { r });
        }
    }
}
