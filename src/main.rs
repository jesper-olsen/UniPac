use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    style::{self, Stylize},
    terminal, //QueueableCommand, Result,
};

use rand::random;
use rand::rngs::ThreadRng;
use rand::seq::IteratorRandom;
use std::collections::HashMap;
use std::io::{self, stdout, Write};
use std::{thread, time};

use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
};

const MAX_PACMAN_LIVES: u32 = 6;
const WIDTH: usize = 28;
const WIDTHM1: usize = WIDTH - 1;
const GHOSTS_INIT: [Ghost; 4] = [
    Ghost::new(10 * WIDTH + 12),
    Ghost::new(10 * WIDTH + 14),
    Ghost::new(11 * WIDTH + 12),
    Ghost::new(11 * WIDTH + 14),
];

fn pct(n: u8) -> bool {
    random::<u8>() % 100 < n
}

fn level2bonus(level: u32) -> (&'static str, u32) {
    match level {
        0 => ("\u{1F352}", 100),        // cherries
        1 => ("\u{1F353}", 300),        // strawberry
        2 | 3 => ("\u{1F351}", 500),    // peach
        4 | 5 => ("\u{1F34E}", 700),    // red apple
        6 | 7 => ("\u{1F347}", 1000),   // grapes
        8 | 9 => ("\u{1F680}", 2000),   // rocket ship (Galaxian)
        10 | 11 => ("\u{1F514}", 3000), // bell
        _ => ("\u{1F511}", 5000),       // key
    }
}

static LEVEL1MAP: &str = concat!(
    "############################",
    "#............##............#",
    "#.####.#####.##.#####.####.#",
    "#P####.#####.##.#####.####P#",
    "#..........................#",
    "#.####.##.########.##.####.#",
    "#......##....##....##......#",
    "######.##### ## #####.######",
    "     #.##          ##.#     ",
    "     #.## ###--### ##.#     ",
    "######.## # HHHH # ##.######",
    "      .   # HHHH #   .      ",
    "######.## # HHHH # ##.######",
    "     #.## ######## ##.#     ",
    "     #.##    $     ##.#     ",
    "######.## ######## ##.######",
    "#............##............#",
    "#.####.#####.##.#####.####.#",
    "#P..##................##..P#",
    "###.##.##.########.##.##.###",
    "#......##....##....##......#",
    "#.##########.##.##########.#",
    "#..........................#",
    "############################",
);

fn tunnel(pos: usize) -> bool {
    (11 * WIDTH..=11 * WIDTH + 5).contains(&pos)
        || (11 * WIDTH + 22..=11 * WIDTH + WIDTHM1).contains(&pos)
}

fn slowdown_ghost(g: &Ghost, level: u32) -> bool {
    match level {
        0 if tunnel(g.pos) => pct(60),
        0 if g.edible_duration > 0 => pct(60),
        0 => pct(25),
        1..=3 if tunnel(g.pos) => pct(55),
        1..=3 if g.edible_duration > 0 => pct(50),
        1..=3 => pct(15),
        _ if tunnel(g.pos) => pct(50),
        _ if g.edible_duration > 0 => pct(45),
        _ => pct(5),
    }
}

fn index2xy(i: usize) -> (u16, u16) {
    (
        (i % WIDTH).try_into().unwrap(),
        (i / WIDTH).try_into().unwrap(),
    )
}

fn index2xy_usize(i: usize) -> (usize, usize) {
    (i % WIDTH, i / WIDTH)
}

const fn xy2index(col: usize, row: usize) -> usize {
    row * WIDTH + col
}

static MARQUEE: &str = "--------- Plato --------------------------------- \
    Socrates: Greetings, my dear Plato. What is it that captures your attention so? \
    \
    Plato: Ah, Socrates, I have been observing a most intriguing phenomenon. \
    Have you heard of the game known as Pacman? \
    \
    Socrates: Pacman? Pray tell, what is this game that has caught your interest? \
    \
    Plato: It is a game of strategy and wit, where a yellow character named Pacman \
    must navigate through a maze and consume small pellets while being pursued by a group of ghostly apparitions. \
    \
    Socrates: Intriguing indeed. And what have you observed about this game? \
    \
    Plato: I have observed that the struggle between Pacman and the ghosts is one of balance and tension. \
    Pacman must navigate the maze while being pursued by the ghosts, who seek to impede his progress and \
    \
    Socrates: And what of Pacman's own abilities? Does he not possess any power to defeat the ghosts? \
    \
    Plato: Indeed, he does. At times, Pacman is able to consume a power pellet, which imbues him with the \
    ability to turn the tables on the ghosts and consume them in turn. \
    \
    Socrates: Fascinating! It would seem that this struggle is one of strategy and timing, with each side \
    seeking to outmaneuver the other. \
    \
    Plato: Precisely, my dear Socrates. The struggle between Pacman and the ghosts is a microcosm of the \
    larger struggle between order and chaos, between the forces of light and darkness. \
    \
    Socrates: Indeed, my dear Plato. This game provides us with a valuable lesson: that even in the most \
    seemingly trivial of contests, there is the potential for great insight and understanding.
    ------------------------------------------------------------------------------ ";

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

use Direction::*;

impl Direction {
    pub fn opposite(&self) -> Direction {
        match self {
            Right => Left,
            Left => Right,
            Down => Up,
            Up => Down,
        }
    }
}

enum GameState {
    UserQuit,
    SheetComplete,
    LifeLost,
}

#[derive(PartialEq, Clone, Copy)]
enum GhostState {
    Home,
    Gateway,
    Outside,
    Dead,
}

struct Ghost {
    pos: usize,
    ghost_state: GhostState,
    edible_duration: u128,
    direction: Direction,
}

impl Ghost {
    const fn new(pos: usize) -> Self {
        Ghost {
            pos,
            direction: Left,
            edible_duration: 0,
            ghost_state: GhostState::Home,
        }
    }

    fn ghost_moves(&self, board: &[char], target: usize) -> Vec<(Direction, usize)> {
        let (col, row) = index2xy_usize(self.pos);
        let (tcol, trow) = index2xy_usize(target);
        let mut l: Vec<(usize, Direction, usize)> = [Right, Left, Down, Up]
            .into_iter()
            .map(move |d| match (d, col) {
                (Right, WIDTHM1) => (Right, row * WIDTH),
                (Right, _) => (Right, self.pos + 1),
                (Left, 0) => (Left, row * WIDTH + WIDTH - 1),
                (Left, _) => (Left, self.pos - 1),
                (Down, _) => (Down, self.pos + WIDTH),
                (Up, _) => (Up, self.pos - WIDTH),
            })
            .filter(|(_d, p)| matches!(board[*p], 'P' | ' ' | '.' | '$'))
            .filter(|(d, _p)| self.edible_duration > 0 || *d != self.direction.opposite()) // not go back
            .map(|(d, p)| {
                let (ncol, nrow) = index2xy_usize(p);
                let dst = tcol.abs_diff(ncol) + trow.abs_diff(nrow);
                (dst, d, p)
            })
            .collect();
        //l.sort();
        l.sort_unstable_by_key(|(distance, _, _)| *distance);
        l.into_iter().map(|(_, dir, pos)| (dir, pos)).collect()
    }
}

struct Player {
    pos: usize,
    dead: bool,
    last_input_direction: Direction,
    moving: Direction,
    anim_frame: usize,
    timecum: u128,
}

const PLAYER_INIT: Player = Player {
    pos: xy2index(14, 18),
    dead: false,
    last_input_direction: Left,
    moving: Left,
    anim_frame: 0,
    timecum: 0,
};

struct Game {
    board: [char; LEVEL1MAP.len()],
    mq_idx: usize,
    timecum: u128,
    dots_left: u32,
    high_score: u32,
    lives: u32,
    player: Player,
    level: u32,
    ghosts: [Ghost; 4],
    pill_duration: u128,
    fruit_duration: u128,
    next_ghost_score: u32,
    score: u32,
    am: AM,
    rng: ThreadRng,
}

#[derive(PartialEq)]
enum Period {
    Scatter,
    Chase,
}

fn period(level: u32, timecum: u128) -> Period {
    match timecum {
        0..=6999 => Period::Scatter,
        7000..=26999 => Period::Chase,
        27000..=33999 => Period::Scatter,
        34000..=53999 => Period::Chase,
        54000..=58999 => Period::Scatter,
        59000..=78999 if level == 0 => Period::Chase,
        79000..=83999 if level == 0 => Period::Scatter,
        _ => Period::Chase,
    }
}

impl Game {
    fn new() -> Self {
        let manager = AudioManager::<CpalBackend>::new(AudioManagerSettings::default())
            .expect("Failed to create AM");

        let mut sounds = HashMap::new();
        for s in [
            "Audio/die.ogg",
            "Audio/eatpill.ogg",
            "Audio/eatghost.ogg",
            "Audio/extra_lives.ogg",
            "Audio/opening_song.ogg",
        ]
        .iter()
        {
            let snd = StaticSoundData::from_file(s, StaticSoundSettings::default())
                .expect("Failed to load sound");
            sounds.insert(s.to_string(), snd);
        }

        let mut game = Game {
            timecum: 0,
            mq_idx: 0,
            ghosts: GHOSTS_INIT,
            pill_duration: 6000,
            level: 0,
            board: LEVEL1MAP.chars().collect::<Vec<_>>().try_into().unwrap(),
            dots_left: 0,
            high_score: 9710,
            lives: 3,
            player: PLAYER_INIT,
            fruit_duration: 0,
            next_ghost_score: 0,
            score: 0,
            am: AM { manager, sounds },
            rng: rand::thread_rng(),
        };

        game.initialise();
        game
    }

    fn repopulate_board(&mut self) {
        self.board = LEVEL1MAP.chars().collect::<Vec<_>>().try_into().unwrap();
        self.dots_left = self.board.iter().filter(|&c| *c == '.').count() as u32;
        self.dots_left += 2; // +2 pseudo dots for fruit bonuses
    }

    fn initialise(&mut self) {
        self.repopulate_board();
        self.ghosts = GHOSTS_INIT;
    }

    fn ghosts_are_edible(&mut self, duration: u128) {
        self.ghosts
            .iter_mut()
            .filter(|g| g.ghost_state == GhostState::Outside)
            .for_each(|g| {
                g.edible_duration += duration;
            })
    }

    fn check_player_vs_ghosts(&mut self) -> io::Result<()> {
        for g in self.ghosts.iter_mut() {
            if g.ghost_state != GhostState::Dead && g.pos == self.player.pos {
                if g.edible_duration == 0 {
                    self.player.dead = true;
                } else {
                    self.am.play("Audio/eatghost.ogg".to_string())?;
                    self.score += self.next_ghost_score;

                    draw_message_at(g.pos, &format!("{}", self.next_ghost_score))?;
                    thread::sleep(time::Duration::from_millis(150));

                    self.next_ghost_score *= 2;
                    g.ghost_state = GhostState::Dead;
                    g.edible_duration = 0;
                }
            }
        }
        Ok(())
    }

    fn update_fruit(&mut self, telaps: u128) {
        self.timecum += telaps;
        // if self.timecum > 500 {
        //     self.timecum = 0;
        // }
        self.fruit_duration = self.fruit_duration.saturating_sub(telaps);
    }

    fn update_ghosts(&mut self, telaps: u128) {
        // const _SCATTER_TARGET: [usize; 4] = [
        //     0 * WIDTH + 2,
        //     0 * WIDTH + WIDTH - 3,
        //     24 * WIDTH + 0,
        //     24 * WIDTH + WIDTH - 1,
        // ];
        // Calc chase mode target pos for Pinky, Blinky, Inky & Clyde
        let mut chase_target: [usize; 4] = [self.player.pos; 4];
        // Pinky - target pacman pos
        // Blinky - target 4 squares away from pacman
        let (pcol, prow) = index2xy_usize(self.player.pos);
        chase_target[1] = match self.player.moving {
            Left => {
                let c: i32 = std::cmp::max(0, pcol as i32 - 4);
                prow * WIDTH + c as usize
            }
            Right => {
                let c: i32 = std::cmp::min((pcol + 4) as i32, WIDTHM1 as i32);
                prow * WIDTH + c as usize
            }
            Up => {
                let r: i32 = std::cmp::max(0, prow as i32 - 4);
                r as usize * WIDTH + pcol
            }
            Down => self.player.pos + 4 * WIDTH,
        };
        // Inky - target average of pacman pos and Blinky
        let (pcol, prow) = index2xy_usize(self.player.pos);
        let (bcol, brow) = index2xy_usize(self.ghosts[1].pos);
        let (tcol, trow) = ((pcol + bcol) / 2, (prow + brow) / 2);

        chase_target[2] = trow * WIDTH + tcol;

        // Clyde - target pacman if less than 8 squares away - otherwise target corner
        let (bcol, brow) = index2xy_usize(self.ghosts[3].pos);
        let dist = bcol.abs_diff(pcol).pow(2) + brow.abs_diff(prow).pow(2);
        if dist >= 64 {
            chase_target[3] = xy2index(0, 2)
        }

        self.ghosts.iter_mut().enumerate().for_each(|(gidx, g)| {
            g.edible_duration = if g.edible_duration < telaps {
                0
            } else {
                g.edible_duration - telaps
            };
            match g.ghost_state {
                GhostState::Home => {
                    let a: [usize; 4] = [g.pos - 1, g.pos + 1, g.pos - WIDTH, g.pos + WIDTH];
                    let idx = a[random::<usize>() % a.len()];
                    match self.board[idx] {
                        'H' => g.pos = idx,
                        '-' => {
                            g.pos = idx;
                            g.ghost_state = GhostState::Gateway;
                        }
                        _ => (),
                    }
                }
                GhostState::Gateway => {
                    g.pos -= WIDTH;
                    g.ghost_state = GhostState::Outside;
                    g.direction = match random::<u8>() % 2 {
                        0 => Left,
                        _ => Right,
                    }
                }

                GhostState::Dead => {
                    // if at house gate - go in
                    if g.pos == 8 * WIDTH + 13 || g.pos == 8 * WIDTH + 14 {
                        g.pos += WIDTH;
                        g.direction = Down;
                    } else if g.pos == 9 * WIDTH + 13 || g.pos == 9 * WIDTH + 14 {
                        g.pos += WIDTH;
                        g.ghost_state = GhostState::Home;
                    } else {
                        (g.direction, g.pos) = g.ghost_moves(&self.board, 8 * WIDTH + 13)[0];
                    }
                }
                GhostState::Outside => {
                    if !slowdown_ghost(g, self.level) {
                        if g.edible_duration > 0 {
                            //flee pacman
                            (g.direction, g.pos) =
                                *g.ghost_moves(&self.board, self.player.pos).last().unwrap();
                        } else if period(self.level, self.timecum) == Period::Chase {
                            (g.direction, g.pos) =
                                g.ghost_moves(&self.board, chase_target[gidx])[0];
                        } else {
                            // scatter mode
                            (g.direction, g.pos) = g
                                .ghost_moves(&self.board, g.pos)
                                .into_iter()
                                .choose(&mut self.rng)
                                .unwrap();
                        }
                    }
                } // Outside
            } // match ghost_state
        })
    }

    fn update(&mut self, dur: u128) -> io::Result<()> {
        self.update_player(dur)?;
        self.check_player_vs_ghosts()?;
        self.update_ghosts(dur);
        self.check_player_vs_ghosts()?;
        self.update_fruit(dur);
        Ok(())
    }

    fn next_player_pos(&self, d: Direction) -> usize {
        let col = self.player.pos % WIDTH;
        match d {
            Right => match col {
                WIDTHM1 => self.player.pos - col, // tunnel
                _ => self.player.pos + 1,
            },
            Left => match col {
                0 => self.player.pos + (WIDTHM1 - col), // tunnel
                _ => self.player.pos - 1,
            },
            Down => self.player.pos + WIDTH,
            Up => self.player.pos - WIDTH,
        }
    }

    fn update_player(&mut self, telaps: u128) -> io::Result<()> {
        self.player.timecum += telaps;
        while self.player.timecum > 100 {
            self.player.timecum -= 100;
            self.player.anim_frame = (self.player.anim_frame + 1) % 6;
        }

        let prev_score = self.score;
        let mut idx = self.next_player_pos(self.player.last_input_direction);
        match self.board[idx] {
            'P' | ' ' | '.' | '$' => {
                self.player.moving = self.player.last_input_direction;
            }
            _ => {
                idx = self.next_player_pos(self.player.moving);
            }
        }

        if matches!(self.board[idx], 'P' | ' ' | '.' | '$') {
            self.player.pos = idx;
            match self.board[idx] {
                '.' => {
                    self.score += 10;
                    self.dots_left -= 1;
                    self.board[idx] = ' ';
                }
                'P' => {
                    self.am.play("Audio/eatpill.ogg".to_string())?;
                    self.board[idx] = ' ';
                    self.ghosts_are_edible(self.pill_duration);
                    self.score += 50;
                    self.next_ghost_score = 200;
                }
                '$' if self.fruit_duration > 0 => {
                    self.am.play("Audio/eatpill.ogg".to_string())?;
                    let bonus = level2bonus(self.level).1;
                    self.score += bonus;
                    self.fruit_duration = 0;

                    draw_message(&format!("{}", bonus), false)?;
                    thread::sleep(time::Duration::from_millis(150));
                }
                _ => (),
            }
        }

        if prev_score < 10000 && self.score >= 10000 && self.lives < MAX_PACMAN_LIVES {
            self.lives += 1;
            self.am.play("Audio/extra_lives.ogg".to_string())?;
        }

        if self.score > self.high_score {
            self.high_score = self.score;
        }
        Ok(())
    } // update_player
} // impl Game

fn init_render() -> io::Result<()> {
    crossterm::queue!(
        stdout(),
        style::ResetColor,
        terminal::Clear(terminal::ClearType::All),
        terminal::EnterAlternateScreen,
        cursor::Hide,
        cursor::MoveTo(0, 0)
    )?;
    terminal::enable_raw_mode()?;
    Ok(())
}

fn close_render() -> io::Result<()> {
    crossterm::queue!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        terminal::LeaveAlternateScreen,
        cursor::Show,
        cursor::MoveTo(0, 0)
    )?;
    terminal::disable_raw_mode()
}

fn draw_end_game() -> io::Result<()> {
    draw_message("GAME  OVER", true)
}

fn draw_message(s: &str, blink: bool) -> io::Result<()> {
    let col: u16 = ((WIDTH - s.len()) / 2).try_into().unwrap();
    let s1 = match blink {
        true => s.bold().slow_blink(),
        false => s.bold(),
    };
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, 14),
        style::PrintStyledContent(s1.bold())
    )?;
    stdout().flush()
}

fn draw_message_at(pos: usize, s: &str) -> io::Result<()> {
    let (mut col, row) = index2xy(pos);
    if col > WIDTH as u16 - 4 {
        col = WIDTH as u16 - 4;
    }
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, row),
        style::PrintStyledContent(s.bold())
    )?;
    stdout().flush()
}

fn draw_start_game() -> io::Result<()> {
    draw_message("READY!", false)?;
    thread::sleep(time::Duration::from_millis(1000));
    Ok(())
}

fn centered_x(s: &str) -> u16 {
    let leftedge: u16 = 32;
    let n: u16 = s.len().try_into().unwrap();

    match terminal::size() {
        Ok((cols, _rows)) => (cols - leftedge - n) / 2 + leftedge,
        Err(_) => leftedge,
    }
}

fn another_game() -> io::Result<bool> {
    let s1 = "Another game, squire?";
    let s2 = "Y/N";

    crossterm::queue!(
        stdout(),
        cursor::MoveTo(centered_x(s1), 12),
        style::PrintStyledContent(s1.red()),
        cursor::MoveTo(centered_x(s2), 14),
        style::PrintStyledContent(s2.red()),
    )?;
    stdout().flush()?;

    loop {
        match read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) if matches!(c, 'y' | 'Y') => return Ok(true),
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) if matches!(c, 'n' | 'N') => return Ok(false),
            _ => (),
        }
    }
}

fn pause() -> io::Result<()> {
    draw_message("PAUSED", false)?;
    loop {
        if let Ok(Event::Key(KeyEvent {
            code: KeyCode::Char(' '),
            ..
        })) = read()
        {
            return Ok(());
        }
    }
}

fn render_game_info() -> io::Result<()> {
    let s1: &str = "UniPac - Unicode-powered Pacman";
    let s2 = "Rusty Edition 2023 ";

    crossterm::queue!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(centered_x(s1), 2),
        style::PrintStyledContent(s1.cyan()),
        cursor::MoveTo(centered_x(s2), 3),
        style::PrintStyledContent(s2.yellow()),
    )
}

fn animate_dead_player(game: &Game) -> io::Result<()> {
    let sz_anim = "|Vv_.+*X*+. ";
    for ch in sz_anim.chars() {
        draw_board(game, false)?;

        let (col, row) = index2xy(game.player.pos);
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(col, row),
            style::PrintStyledContent(ch.bold().yellow()),
        )?;
        stdout().flush()?;

        thread::sleep(time::Duration::from_millis(150));
    }
    Ok(())
}

fn render_rhs(game: &Game) -> io::Result<()> {
    // draw lives - ascii art, one pacman for each
    // let pacimg = ["/-\\", "|'<", "\\_/", "   ", "   ", "   "];
    // // need to remove the old pacman character in some cases
    // for i in 0..MAX_PACMAN_LIVES {
    //     for j in 0..3 {
    //         let q = if game.lives > i { 0 } else { 3 };
    //         crossterm::queue!(
    //             stdout(),
    //             cursor::MoveTo(
    //                 (i * 5 + 30).try_into().unwrap(),
    //                 (16 + j).try_into().unwrap()
    //             ),
    //             style::PrintStyledContent(pacimg[(j + q) as usize].bold().yellow()),
    //         )
    //         .ok();
    //     }
    // }

    let s = if period(game.level, game.timecum) == Period::Chase {
        "\u{1F4A1}" // light bulb
    } else {
        "  "
    };

    crossterm::queue!(stdout(), cursor::MoveTo(30, 23), style::Print(s)).ok();

    let i = centered_x("Score : 123456"); // get a pos base on av score digits
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(i, 5.try_into().unwrap()),
        style::PrintStyledContent(format!("Score  : {}", game.score).bold().white()),
        cursor::MoveTo(i, 6.try_into().unwrap()),
        style::PrintStyledContent(format!("High   : {}", game.high_score).bold().white()),
        cursor::MoveTo(i, 8.try_into().unwrap()),
        style::PrintStyledContent(format!("Level  : {}", game.level + 1).bold().white()),
    )?;

    draw_message_at(25 * WIDTH - 1, level2bonus(game.level).0)?;

    let s = vec!['\u{1F642}'; game.lives as usize];
    let s1 = vec![' '; MAX_PACMAN_LIVES as usize - s.len()];
    let s2: String = s.into_iter().chain(s1).collect::<String>();
    draw_message_at(24 * WIDTH, &s2)?;

    // scroll marquee
    let (cols, rows) = match terminal::size() {
        Ok((cols, rows)) => (cols, rows),
        Err(_) => (0, 0), // panic!
    };

    // rediculous - but here we go
    let i: u16 = if let Ok(width) = WIDTH.try_into() {
        if cols > width {
            0
        } else {
            width
        }
    } else {
        u16::MAX
    };

    let q: u16 = cols - i;

    let i1: usize = game.mq_idx % MARQUEE.len();
    let t: usize = q as usize + game.mq_idx;
    let i2: usize = t % MARQUEE.len();
    crossterm::execute!(stdout(), cursor::MoveTo(i, rows - i))?;
    if i1 < i2 {
        crossterm::execute!(stdout(), style::PrintStyledContent(MARQUEE[i1..i2].white()))?;
    } else {
        crossterm::execute!(
            stdout(),
            style::PrintStyledContent(
                format!("{}{}", &MARQUEE[i1..MARQUEE.len() - 1], &MARQUEE[0..i2]).white()
            )
        )?
    }
    Ok(())
}

fn draw_board(game: &Game, bold: bool) -> io::Result<()> {
    for (i, c) in game.board.iter().enumerate() {
        let s = match *c {
            '#' => "#".blue(),
            '.' => ".".white(),
            'P' => "*".slow_blink(),
            _ => " ".white(),
        };
        let s = if bold { s.bold() } else { s };
        let (col, row) = index2xy(i);
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(col, row),
            style::PrintStyledContent(s),
        )?;
    }

    // print fruit separately - because not rendered correctly otherwise (is wider than one cell)
    if game.fruit_duration > 0 {
        let fruit = level2bonus(game.level).0;
        for (i, c) in game.board.iter().enumerate() {
            if *c == '$' {
                let (col, row) = index2xy(i);
                crossterm::queue!(stdout(), cursor::MoveTo(col, row), style::Print(fruit),)?;
            }
        }
    }
    Ok(())
}

fn flash_board(game: &Game) -> io::Result<()> {
    for i in 0..10 {
        draw_board(game, i % 2 == 0)?;
        stdout().flush()?;
        thread::sleep(time::Duration::from_millis(300));
    }
    Ok(())
}

fn draw_player(game: &Game) -> io::Result<()> {
    let sz_anim = match game.player.last_input_direction {
        Left => ['}', ')', '>', '-', '>', ')'],
        Right => ['{', '(', '<', '-', '<', '('],
        Up => ['V', 'V', 'V', 'V', '|', '|'],
        Down => ['^', '^', '^', '^', '|', '|'],
    };
    let (col, row) = index2xy(game.player.pos);
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, row),
        style::PrintStyledContent(sz_anim[game.player.anim_frame].bold().yellow()),
    )
}

fn draw_ghosts(game: &Game) -> io::Result<()> {
    //"\u{1F631}".rapid_blink() // looks bad
    for (i, g) in game.ghosts.iter().enumerate() {
        let s = match (g.ghost_state, game.board[g.pos] != 'H', i) {
            (GhostState::Dead, _, _) => "\u{1F440}", // Eyes
            (_, true, _) if (1..2000).contains(&g.edible_duration) => "\u{1F47D}", // Alien
            (_, true, _) if g.edible_duration > 0 => "\u{1F631}", // Scream
            (_, _, 0) => "\u{1F47A}",                // Goblin
            (_, _, 1) => "\u{1F479}",                // Ogre
            (_, _, 2) => "\u{1F47B}",                // Ghost
            (_, _, _) => "\u{1F383}",                // Jack-O-Lantern
        };
        let (col, row) = index2xy(g.pos);
        crossterm::queue!(stdout(), cursor::MoveTo(col, row), style::Print(s),)?;
    }
    Ok(())
}

//  The animated death and flashing screen happen syncronously. To be done
//  correctly, they should be pseudo-event driven like the rest of the program.
fn draw_dynamic(game: &Game) -> io::Result<()> {
    draw_board(game, false)?;
    draw_player(game)?;
    draw_ghosts(game)?;
    render_rhs(game)?;
    stdout().flush()
}

fn game_loop(game: &mut Game) -> io::Result<GameState> {
    loop {
        let start = time::Instant::now();

        // adjust overall speed by level
        let mut delta = match game.level {
            0 => 140,
            1..=3 => 130,
            _ => 120,
        };
        // faster if power pill eaten
        if game.ghosts.iter().filter(|g| g.edible_duration > 0).count() > 0 {
            delta -= 20;
        }
        thread::sleep(time::Duration::from_millis(delta));

        if let Ok(true) = poll(time::Duration::from_millis(10)) {
            game.player.last_input_direction = match read() {
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                })) => return Ok(GameState::UserQuit),
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('v'),
                    ..
                })) => {
                    game.ghosts_are_edible(game.pill_duration); // cheat
                    game.player.last_input_direction
                }

                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Left,
                    ..
                })) => Left,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Right,
                    ..
                })) => Right,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Up, ..
                })) => Up,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    ..
                })) => Down,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char(' '),
                    ..
                })) => {
                    pause()?;
                    game.player.last_input_direction
                }
                _ => game.player.last_input_direction,
            };
        }
        game.mq_idx = (game.mq_idx + 1) % MARQUEE.len(); // scroll marquee

        game.update((time::Instant::now() - start).as_millis())?;
        draw_dynamic(game)?;

        if game.player.dead {
            return Ok(GameState::LifeLost);
        }

        match game.dots_left {
            0 => return Ok(GameState::SheetComplete),
            74 | 174 => {
                game.fruit_duration = 1000 * (10 + random::<u128>() % 3);
                game.dots_left -= 1;
            }
            _ => (),
        }
    }
}

struct AM {
    manager: AudioManager,
    sounds: HashMap<String, StaticSoundData>,
}

impl AM {
    fn play(&mut self, name: String) -> Result<StaticSoundHandle, std::io::Error> {
        self.manager
            .play(self.sounds.get(&name).unwrap().clone())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

fn main_game() -> io::Result<()> {
    let mut game = Game::new();
    render_game_info()?;
    loop {
        draw_dynamic(&game)?;
        draw_start_game()?;
        thread::sleep(time::Duration::from_millis(200));
        match game_loop(&mut game)? {
            GameState::UserQuit => return Ok(()),
            GameState::SheetComplete => {
                game.am
                    .play("Audio/opening_song.ogg".to_string())
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                flash_board(&game)?;
                game.level += 1;
                game.repopulate_board();
                game.ghosts = GHOSTS_INIT;
                game.player = PLAYER_INIT;
                game.timecum = 0;
            }
            GameState::LifeLost => {
                render_rhs(&game)?;
                game.am
                    .play("Audio/die.ogg".to_string())
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                animate_dead_player(&game)?;
                if game.lives == 0 {
                    return Ok(());
                }
                game.lives -= 1;
                thread::sleep(time::Duration::from_millis(100));
                game.ghosts = GHOSTS_INIT;
                game.player = PLAYER_INIT;
            }
        };
    }
}

fn main() -> io::Result<()> {
    init_render()?;
    while {
        main_game()?;
        draw_end_game()?;
        another_game()?
    } {}
    close_render()
}
