use std::io::Cursor;

use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend, sound::static_sound::StaticSoundData,
};

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub enum Sound {
    Die = 0,
    EatPill,
    EatGhost,
    ExtraLives,
    OpeningSong,
}

// Embedded in the binary, so the game works from any directory and when installed
// with `cargo install`. Order must match `Sound`.
const AUDIO_DATA: [&[u8]; 5] = [
    include_bytes!("../Audio/die.ogg"),
    include_bytes!("../Audio/eatpill.ogg"),
    include_bytes!("../Audio/eatghost.ogg"),
    include_bytes!("../Audio/extra_lives.ogg"),
    include_bytes!("../Audio/opening_song.ogg"),
];

struct Engine {
    manager: AudioManager<DefaultBackend>,
    sounds: Vec<StaticSoundData>,
}

impl Engine {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        let sounds = AUDIO_DATA
            .iter()
            .map(|data| StaticSoundData::from_cursor(Cursor::new(data)))
            .collect::<Result<_, _>>()?;
        Ok(Engine { manager, sounds })
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
            let _ = e.manager.play(e.sounds[name as usize].clone());
        }
    }
}
