# UniPac

Pac-Man for the terminal, in Rust.

![UniPac demo](demo.gif)

* Ghosts alternate between 'scatter' (each heads for its own corner) and 'chase' periods, each with its original targeting.
* Ghosts flee pacman when frightened, and slow down in the tunnel and when frightened.
* When eaten, ghost eyes trace a path back to the ghost house.
* Two 'fruit' bonuses on every level.
* Cornering: steer early and pacman takes the first opening.
* Mazes: shortened pacman maze (24 rows), regular pacman maze (29 rows) and 4 Ms Pacman mazes (31 rows).

Pacman is animated with ASCII symbols, the ghosts and fruit with emoji (Pinky 👺, Blinky 👹, Inky 👻, Clyde 🎃 - Unicode has no ghost symbol). Emoji are two columns wide, so each maze tile is too.

## Run

Needs Rust 1.88+ and a terminal with emoji support, at least 80 columns wide and 33 rows tall.

```
cargo install --git https://github.com/jesper-olsen/UniPac
unipac
```

Or from a clone: `cargo run --release`. `--level N` starts on level N: 1 is the shortened maze, 2 the regular one, 3-6 the Ms Pacman mazes.

On Debian/Ubuntu, building needs `sudo apt install libasound2-dev libdbus-1-dev pkg-config`. Sound is optional: without an audio device the game runs silent.

## Controls

Arrow keys to steer, space to pause, q or Esc to quit.

The high score is saved between runs, in `~/.local/share/unipac` on Linux and `~/Library/Application Support/unipac` on macOS.

## Credits

* Steven Goodwin's [C version](https://github.com/MarquisdeGeek/pacman) of the game.
* Sound assets from Dale Harvey's [JS version](https://github.com/daleharvey/pacman)
* Marquee text by [ChatGPT](https://chat.openai.com/)
* [The Pac-Man Dossier](https://pacman.holenet.info)
* [Ms Pac-Man Walkthrough](https://strategywiki.org/wiki/Ms._Pac-Man/Walkthrough)
