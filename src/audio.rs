use std::io::Cursor;
use std::sync::Mutex;

use rodio::stream::play as play_into_mixer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

/// The navigation sound, embedded so the binary needs no assets on disk.
pub const NAVIGATION_SOUND: &[u8] = include_bytes!("../assets/sound_effect.mp3");

/// The ALSA sink and the currently-playing click, kept alive for the session.
struct NavAudio {
    _sink: MixerDeviceSink,
    player: Mutex<Option<Player>>,
}

static NAV_AUDIO: Mutex<Option<NavAudio>> = Mutex::new(None);

/// Open the default ALSA device and verify the embedded sound decodes. If
/// there is no usable audio output (e.g. the device has no speaker),
/// navigation just stays silent.
pub fn init() {
    let mut sink = match DeviceSinkBuilder::open_default_sink() {
        Ok(sink) => sink,
        Err(err) => {
            eprintln!("audio output init error: {err}");
            return;
        }
    };
    sink.log_on_drop(false);

    if Decoder::new(Cursor::new(NAVIGATION_SOUND)).is_err() {
        eprintln!("navigation sound is not decodable");
        return;
    }

    *NAV_AUDIO.lock().unwrap() = Some(NavAudio {
        _sink: sink,
        player: Mutex::new(None),
    });
}

/// Play the navigation sound. A previous (still sounding) click is cut off,
/// so rapid navigation produces a crisp tick rather than a pile-up.
pub fn play() {
    let guard = NAV_AUDIO.lock().unwrap();
    let Some(nav) = guard.as_ref() else { return };

    if let Some(old) = nav.player.lock().unwrap().take() {
        old.stop();
    }

    match play_into_mixer(nav._sink.mixer(), Cursor::new(NAVIGATION_SOUND)) {
        Ok(player) => *nav.player.lock().unwrap() = Some(player),
        Err(err) => eprintln!("navigation sound playback error: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    #[test]
    fn embedded_sound_decodes() {
        let source = Decoder::new(Cursor::new(NAVIGATION_SOUND)).unwrap();
        let rate = source.sample_rate().get();
        let samples = source.count();
        // ~1 second of audio at the stream rate, in stereo.
        assert!(samples >= rate as usize, "sound is unexpectedly short: {samples} samples");
    }
}