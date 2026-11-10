use std::io::Cursor;

use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend, Tween,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
};

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub enum Sound {
    Die = 0,
    EatPill,
    EatGhost,
    ExtraLives,
    OpeningSong,
    Intermission,
}

// Embedded in the binary, so the game works from any directory and when installed
// with `cargo install`. Order must match `Sound`.
const AUDIO_DATA: [&[u8]; 6] = [
    include_bytes!("../Audio/die.ogg"),
    include_bytes!("../Audio/eatpill.ogg"),
    include_bytes!("../Audio/eatghost.ogg"),
    include_bytes!("../Audio/extra_lives.ogg"),
    include_bytes!("../Audio/opening_song.ogg"),
    include_bytes!("../Audio/intermission.ogg"),
];

struct Engine {
    manager: AudioManager<DefaultBackend>,
    sounds: Vec<StaticSoundData>,
    last: Option<StaticSoundHandle>, // the sound played last
}

impl Engine {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        let sounds = AUDIO_DATA
            .iter()
            .map(|data| StaticSoundData::from_cursor(Cursor::new(data)))
            .collect::<Result<_, _>>()?;
        Ok(Engine {
            manager,
            sounds,
            last: None,
        })
    }
}

/// Sound is optional: without an audio device (e.g. over ssh) the game runs silently.
pub struct AM {
    engine: Option<Engine>,
}

impl Default for AM {
    fn default() -> Self {
        AM {
            engine: Engine::new().ok(),
        }
    }
}

impl AM {
    /// Plays a sound. Does nothing if audio is unavailable; playback errors are ignored
    /// - a sound that can't be played should never end the game.
    pub fn play(&mut self, name: Sound) {
        if let Some(e) = &mut self.engine {
            e.last = e.manager.play(e.sounds[name as usize].clone()).ok();
        }
    }

    /// Stops the sound played last, if it is still playing
    pub fn stop_last(&mut self) {
        if let Some(h) = self.engine.as_mut().and_then(|e| e.last.as_mut()) {
            h.stop(Tween::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The intermission lasts as long as its music: each half of the scene is one play of the tune
    #[test]
    fn intermission_music_matches_the_scene() {
        let data = &AUDIO_DATA[Sound::Intermission as usize];
        let tune = StaticSoundData::from_cursor(Cursor::new(data)).unwrap();
        let ms = tune.duration().as_millis();
        assert!(
            ms.abs_diff(crate::intermission::HALF_MS) < 60,
            "the tune is {ms} ms - update HALF_MS in intermission.rs"
        );
    }
}
