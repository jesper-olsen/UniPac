//! Intermissions ("coffee breaks") between levels, as in the arcade game.
//!
//! A scene is a pure function of time: `sprites` says what is where `ms` milliseconds into it.
//! `tui::intermission` draws it and plays the music.

use crate::board::Direction;

/// The intermissions. So far only the first, played after level 2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    /// Blinky chases pacman off the screen. Then pacman, grown huge, chases a frightened
    /// Blinky back across it.
    One,
}

impl Act {
    pub fn from_number(n: u8) -> Option<Act> {
        match n {
            1 => Some(Act::One),
            _ => None,
        }
    }

    pub const fn duration_ms(self) -> u128 {
        match self {
            Act::One => 2 * HALF_MS,
        }
    }
}

/// The intermission played after completing the level with this (0-based) index, if any.
pub fn act_after(level: u32) -> Option<Act> {
    match level {
        1 => Some(Act::One),
        _ => None,
    }
}

/// The tune is played once for each half of the scene, which lasts as long as the tune
/// (Audio/intermission.ogg: checked by a test in audio.rs).
pub const HALF_MS: u128 = 5200;

/// Width of the giant pacman, in columns
pub const GIANT_W: i32 = 7;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Pacman(Direction),
    Blinky,
    Frightened, // a frightened Blinky
    Giant,      // a giant pacman
}

#[derive(Clone, Copy)]
pub struct Sprite {
    pub kind: Kind,
    pub x: i32, // leftmost column - may be off the stage
    pub anim: usize,
}

impl Sprite {
    pub fn width(&self) -> i32 {
        match self.kind {
            Kind::Pacman(_) => 1,
            Kind::Giant => GIANT_W,
            Kind::Blinky | Kind::Frightened => 2, // emoji: two columns wide
        }
    }

    /// Is any of it on a stage `stage_w` columns wide? An emoji is only shown when it fits
    /// completely.
    pub fn on_stage(&self, stage_w: i32) -> bool {
        match self.kind {
            Kind::Blinky | Kind::Frightened => self.x >= 0 && self.x + 2 <= stage_w,
            _ => self.x + self.width() > 0 && self.x < stage_w,
        }
    }
}

/// What is on a stage `stage_w` columns wide, `ms` milliseconds into the scene.
pub fn sprites(act: Act, ms: u128, stage_w: i32) -> Vec<Sprite> {
    match act {
        Act::One => act_one(ms, stage_w),
    }
}

fn act_one(ms: u128, w: i32) -> Vec<Sprite> {
    if ms >= Act::One.duration_ms() {
        return vec![];
    }
    let t = (ms % HALF_MS) as f64 / HALF_MS as f64; // how far into this half of the scene
    let lerp = |from: i32, to: i32| (from as f64 + (to - from) as f64 * t).round() as i32;
    if ms < HALF_MS {
        // Pacman runs left, off the stage. Blinky is behind him, and gradually gains on him
        // without ever catching up.
        let pacman = lerp(w + 2, -8);
        let gap = (14.0 - 10.0 * t * t).round() as i32;
        vec![
            Sprite {
                kind: Kind::Pacman(Direction::Left),
                x: pacman,
                anim: (ms / 80) as usize % 6,
            },
            Sprite {
                kind: Kind::Blinky,
                x: pacman + gap,
                anim: 0,
            },
        ]
    } else {
        // Now pacman is giant, and chases a frightened Blinky to the right, off the stage
        let giant = lerp(-15, w);
        let gap = (6.0 - 2.0 * t).round() as i32;
        vec![
            Sprite {
                kind: Kind::Giant,
                x: giant,
                anim: [0, 1, 2, 1][(ms / 100) as usize % 4],
            },
            Sprite {
                kind: Kind::Frightened,
                x: giant + GIANT_W + gap,
                anim: 0,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 56; // the stage: the maze, 28 tiles of 2 columns

    fn first(ms: u128, kind: fn(&Kind) -> bool) -> Sprite {
        *sprites(Act::One, ms, W)
            .iter()
            .find(|s| kind(&s.kind))
            .expect("sprite")
    }

    #[test]
    fn intermission_after_level_2() {
        assert_eq!(act_after(0), None);
        assert_eq!(act_after(1), Some(Act::One)); // level 2, 0-based
        assert_eq!(act_after(2), None);
        assert_eq!(Act::from_number(1), Some(Act::One));
        assert_eq!(Act::from_number(2), None);
    }

    #[test]
    fn each_half_starts_and_ends_off_the_stage() {
        for ms in [0, HALF_MS - 1, HALF_MS, 2 * HALF_MS - 1] {
            let s = sprites(Act::One, ms, W);
            assert_eq!(s.len(), 2);
            assert!(
                s.iter().all(|s| !s.on_stage(W)),
                "something on stage at {ms}"
            );
        }
        assert!(sprites(Act::One, 2 * HALF_MS, W).is_empty());
        // ... and they are there in between
        for ms in [HALF_MS / 2, HALF_MS + HALF_MS / 2] {
            assert!(sprites(Act::One, ms, W).iter().all(|s| s.on_stage(W)));
        }
    }

    #[test]
    fn blinky_gains_on_pacman_but_never_catches_him() {
        let is_pacman: fn(&Kind) -> bool = |k| matches!(k, Kind::Pacman(_));
        let is_blinky: fn(&Kind) -> bool = |k| *k == Kind::Blinky;
        let mut last_gap = i32::MAX;
        let mut last_x = i32::MAX;
        for ms in (0..HALF_MS).step_by(20) {
            let (p, b) = (first(ms, is_pacman), first(ms, is_blinky));
            let gap = b.x - p.x;
            assert!(gap >= 4, "Blinky too close at {ms}: {gap}");
            assert!(gap <= last_gap, "the gap grows at {ms}");
            assert!(p.x <= last_x, "pacman turns back at {ms}");
            (last_gap, last_x) = (gap, p.x);
        }
        // he is faster than pacman
        let gap_at = |ms| first(ms, is_blinky).x - first(ms, is_pacman).x;
        assert!(gap_at(0) - gap_at(HALF_MS - 1) >= 8);
    }

    #[test]
    fn the_giant_pacman_chases_a_frightened_blinky_to_the_right() {
        let is_giant: fn(&Kind) -> bool = |k| *k == Kind::Giant;
        let is_blinky: fn(&Kind) -> bool = |k| *k == Kind::Frightened;
        let mut last_x = i32::MIN;
        for ms in (HALF_MS..2 * HALF_MS).step_by(20) {
            let (g, b) = (first(ms, is_giant), first(ms, is_blinky));
            assert!(b.x - (g.x + GIANT_W) >= 4, "the giant is on Blinky at {ms}");
            assert!(g.x >= last_x, "the giant turns back at {ms}");
            last_x = g.x;
        }
    }
}
