#[cfg(feature = "alsa")]
mod imp {
    use std::sync::Mutex;

    const SAMPLE_RATE: u32 = 44100;
    const CHANNELS: u32 = 2;
    const CLICK_DURATION_MS: u32 = 30;
    const CLICK_FREQ: f32 = 800.0;
    const CLICK_VOLUME: f32 = 0.4;

    struct NavAudio {
        pcm: alsa::pcm::PCM,
        click_buffer: Vec<i16>,
    }

    static AUDIO: Mutex<Option<NavAudio>> = Mutex::new(None);

    fn generate_click() -> Vec<i16> {
        let num_samples = (SAMPLE_RATE * CLICK_DURATION_MS / 1000) as usize;
        let mut buf = Vec::with_capacity(num_samples * CHANNELS as usize);

        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE as f32;
            let mut envelope = 1.0 - (i as f32 / num_samples as f32);
            envelope *= envelope;
            let sample =
                (2.0 * std::f32::consts::PI * CLICK_FREQ * t).sin() * CLICK_VOLUME * envelope;
            let s = (sample * 32767.0) as i16;
            buf.push(s);
            buf.push(s);
        }
        buf
    }

    pub fn audio_init() {
        let pcm = match alsa::pcm::PCM::new("default", alsa::Direction::Playback, false) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("audio init error: {e}");
                return;
            }
        };

        {
            let hwp = match alsa::pcm::HwParams::any(&pcm) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("audio init hw_params error: {e}");
                    return;
                }
            };
            if let Err(e) = hwp.set_channels(CHANNELS) {
                eprintln!("audio init channels error: {e}");
                return;
            }
            if let Err(e) = hwp.set_rate(SAMPLE_RATE, alsa::ValueOr::Nearest) {
                eprintln!("audio init rate error: {e}");
                return;
            }
            if let Err(e) = hwp.set_format(alsa::pcm::Format::s16()) {
                eprintln!("audio init format error: {e}");
                return;
            }
            if let Err(e) = hwp.set_access(alsa::pcm::Access::RWInterleaved) {
                eprintln!("audio init access error: {e}");
                return;
            }
            if let Err(e) = pcm.hw_params(&hwp) {
                eprintln!("audio init set hw_params error: {e}");
                return;
            }
        }

        let nav = NavAudio {
            pcm,
            click_buffer: generate_click(),
        };

        *AUDIO.lock().unwrap() = Some(nav);
        eprintln!("audio: initialized");
    }

    pub fn audio_play() {
        let mut lock = AUDIO.lock().unwrap();
        if let Some(ref mut audio) = *lock {
            let _ = audio.pcm.prepare();
            let io = audio.pcm.io_i16();
            if let Ok(io) = io {
                let _ = io.writei(&audio.click_buffer);
            }
        }
    }
}

#[cfg(feature = "alsa")]
pub use imp::{audio_init, audio_play};

#[cfg(not(feature = "alsa"))]
pub fn audio_init() {
    eprintln!("audio: disabled (no ALSA)");
}

#[cfg(not(feature = "alsa"))]
pub fn audio_play() {}
