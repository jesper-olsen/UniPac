use std::{io, path};

use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
};

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub enum Sound {
    Die = 0,
    EatPill,
    EatGhost,
    ExtraLives,
    OpeningSong,
}

const AUDIO_DIR: &str = "Audio";

const AUDIO_FILES: [&str; 5] = [
    "die.ogg",
    "eatpill.ogg",
    "eatghost.ogg",
    "extra_lives.ogg",
    "opening_song.ogg",
];

pub struct AM {
    manager: AudioManager<DefaultBackend>,
    sounds: [StaticSoundData; AUDIO_FILES.len()],
}

impl Default for AM {
    fn default() -> Self {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .expect("Failed to create AM");

        let sounds = AUDIO_FILES.map(|audio_file| {
            let path = path::Path::new(AUDIO_DIR).join(audio_file);

            StaticSoundData::from_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load sound: {path:?}: {e}"))
        });

        AM { manager, sounds }
    }
}

impl AM {
    pub fn play(&mut self, name: Sound) -> Result<StaticSoundHandle, std::io::Error> {
        self.manager
            .play(self.sounds[name as usize].clone())
            .map_err(io::Error::other)
    }
}
