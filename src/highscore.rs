//! The high score is kept in a plain text file in the platform's user data directory:
//! `~/.local/share/unipac/highscore` (Linux), `~/Library/Application Support/unipac/highscore`
//! (macOS) or `%APPDATA%\unipac\highscore` (Windows).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The high score until a better one has been saved.
const DEFAULT: u32 = 9710;

fn path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("unipac").join("highscore"))
}

fn read(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn write(path: &Path, score: u32) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, score.to_string())
}

/// Saved high score, or the default if there is none.
pub fn load() -> u32 {
    path().and_then(|p| read(&p)).unwrap_or(DEFAULT)
}

/// Saves `score` if it beats the stored high score - never lowers it, so two games running
/// at once can't overwrite each other's record. Failing to save is not worth interrupting
/// the game for, so errors are ignored.
pub fn save(score: u32) {
    if let Some(p) = path() {
        save_to(&p, score);
    }
}

fn save_to(path: &Path, score: u32) {
    if score > read(path).unwrap_or(DEFAULT) {
        let _ = write(path, score);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_only_records() {
        let dir = std::env::temp_dir().join(format!("unipac-test-{}", std::process::id()));
        let file = dir.join("sub").join("highscore"); // directories are created on demand
        let _ = fs::remove_dir_all(&dir);

        save_to(&file, DEFAULT); // not a record: nothing is written
        assert_eq!(read(&file), None);
        save_to(&file, 12000);
        assert_eq!(read(&file), Some(12000));
        save_to(&file, 11000); // lower scores don't overwrite
        assert_eq!(read(&file), Some(12000));
        save_to(&file, 12500);
        assert_eq!(read(&file), Some(12500));

        fs::write(&file, "not a number").unwrap(); // a damaged file is treated as missing
        assert_eq!(read(&file), None);
        save_to(&file, 10000);
        assert_eq!(read(&file), Some(10000));

        fs::remove_dir_all(&dir).unwrap();
    }
}
