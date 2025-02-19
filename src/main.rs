use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    style::{self, Stylize},
    terminal, //QueueableCommand, Result,
};

// TODO - speed function of level

use std::io::{stdout, Write};

use rand::random;
use rand::seq::IteratorRandom;

use std::collections::HashMap;
use std::time::Instant;
use std::{thread, time};

use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
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

fn level2fruit(level: u32) -> (&'static str, u32) {
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

fn opposite_direction(dir: Direction) -> Direction {
    match dir {
        Direction::Right => Direction::Left,
        Direction::Left => Direction::Right,
        Direction::Down => Direction::Up,
        Direction::Up => Direction::Down,
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

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

enum GameState {
    UserQuit,
    SheetComplete,
    LifeLost,
}

#[derive(PartialEq)]
enum GhostState {
    Home,
    Gateway,
    Outside,
    Dead,
}

struct Ghost {
    pos: usize,
    ghost_state: GhostState,
    edible_duration: u32,
    direction: Direction,
}

impl Ghost {
    const fn new(pos: usize) -> Self {
        Ghost {
            pos,
            direction: Direction::Left,
            edible_duration: 0,
            ghost_state: GhostState::Home,
        }
    }
}

struct Player {
    pos: usize,
    dead: bool,
    last_input_direction: Direction,
    moving: Direction,
    anim_frame: usize,
    timecum: u32,
}

const PLAYER_INIT: Player = Player {
    pos: 18 * WIDTH + 14,
    dead: false,
    last_input_direction: Direction::Left,
    moving: Direction::Left,
    anim_frame: 0,
    timecum: 0,
};

struct Game {
    board: Vec<char>,
    mq_idx: usize,
    timecum: u32,
    dots_left: u32,
    high_score: u32,
    lives: u32,
    player: Player,
    level: u32,
    ghosts: [Ghost; 4],
    pill_duration: u32,
    fruit_duration: u32,
    next_ghost_score: u32,
    score: u32,
    am: AM,
}

#[derive(PartialEq)]
enum Period {
    Scatter,
    Chase,
}

fn period(level: u32, timecum: u32) -> Period {
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

fn ghost_moves(pos: usize) -> impl Iterator<Item = (Direction, usize)> {
    [
        Direction::Right,
        Direction::Left,
        Direction::Down,
        Direction::Up,
    ]
    .iter()
    .map(move |d| {
        let (col, row) = index2xy_usize(pos);
        match (d, col) {
            (Direction::Right, WIDTHM1) => (Direction::Right, row * WIDTH),
            (Direction::Right, _) => (Direction::Right, pos + 1),
            (Direction::Left, 0) => (Direction::Left, row * WIDTH + WIDTH - 1),
            (Direction::Left, _) => (Direction::Left, pos - 1),
            (Direction::Down, _) => (Direction::Down, pos + WIDTH),
            (Direction::Up, _) => (Direction::Up, pos - WIDTH),
        }
    })
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
            "Audio/extra lives.ogg",
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
            board: LEVEL1MAP.chars().collect(),
            dots_left: 0,
            high_score: 9710,
            lives: 3,
            player: PLAYER_INIT,
            fruit_duration: 0,
            next_ghost_score: 0,
            score: 0,
            am: AM { manager, sounds },
        };

        game.initialise();
        game
    }
    fn repopulate_board(&mut self) {
        self.board = LEVEL1MAP.chars().collect();
        self.dots_left = self
            .board
            .iter()
            .filter(|&c| *c == '.')
            .count()
            .try_into()
            .unwrap();
        self.dots_left += 2; // +2 pseudo dots for fruit bonuses
    }

    fn initialise(&mut self) {
        self.repopulate_board();
        self.ghosts = GHOSTS_INIT;
    }

    fn ghosts_are_edible(&mut self, duration: u32) {
        self.ghosts
            .iter_mut()
            .filter(|g| g.ghost_state == GhostState::Outside)
            .for_each(|g| {
                g.edible_duration += duration;
            })
    }

    fn check_player_vs_ghosts(&mut self) {
        self.ghosts
            .iter_mut()
            .filter(|g| g.ghost_state != GhostState::Dead && g.pos == self.player.pos)
            .for_each(|g| {
                if g.edible_duration == 0 {
                    self.player.dead = true;
                } else {
                    self.am.play("Audio/eatghost.ogg".to_string());
                    self.score += self.next_ghost_score;

                    draw_message_at(g.pos, &format!("{}", self.next_ghost_score));
                    thread::sleep(time::Duration::from_millis(150));

                    self.next_ghost_score *= 2;
                    g.ghost_state = GhostState::Dead;
                    g.edible_duration = 0;
                }
            })
    }

    fn update_fruit(&mut self, telaps: u32) {
        self.timecum += telaps;
        // if self.timecum > 500 {
        //     self.timecum = 0;
        // }

        if self.fruit_duration > 0 {
            if self.fruit_duration < telaps {
                self.fruit_duration = 0;
            } else {
                self.fruit_duration -= telaps;
            }
        }
    }

    fn update_ghosts(&mut self, telaps: u32) {
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
            Direction::Left => {
                let c: i32 = std::cmp::max(0, pcol as i32 - 4);
                prow * WIDTH + c as usize
            }
            Direction::Right => {
                let c: i32 = std::cmp::min((pcol + 4) as i32, WIDTHM1 as i32);
                prow * WIDTH + c as usize
            }
            Direction::Up => {
                let r: i32 = std::cmp::max(0, prow as i32 - 4);
                r as usize * WIDTH + pcol
            }
            Direction::Down => self.player.pos + 4 * WIDTH,
        };
        // Inky - target average of pacman pos and Blinky
        let (pcol, prow) = index2xy_usize(self.player.pos);
        let (bcol, brow) = index2xy_usize(self.ghosts[1].pos);
        let (tcol, trow) = ((pcol + bcol) / 2, (prow + brow) / 2);

        chase_target[2] = trow * WIDTH + tcol;

        // Clyde - target pacman if less than 8 squares away - otherwise target corner
        let (bcol, brow) = index2xy_usize(self.ghosts[3].pos);
        let bcol: i32 = bcol.try_into().unwrap();
        let brow: i32 = brow.try_into().unwrap();
        let pcol: i32 = pcol.try_into().unwrap();
        let prow: i32 = prow.try_into().unwrap();
        let dist = (bcol - pcol) * (bcol - pcol) + (brow - prow) * (brow - prow);
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
                        0 => Direction::Left,
                        _ => Direction::Right,
                    }
                }

                GhostState::Dead => {
                    // if at house gate - go in
                    // otherwise - don't go back, go in direction of target
                    if g.pos == 8 * WIDTH + 13 || g.pos == 8 * WIDTH + 14 {
                        g.pos += WIDTH;
                        g.direction = Direction::Down;
                    } else if g.pos == 9 * WIDTH + 13 || g.pos == 9 * WIDTH + 14 {
                        g.pos += WIDTH;
                        g.ghost_state = GhostState::Home;
                    } else {
                        (g.direction, g.pos, _) = ghost_moves(g.pos)
                            .filter(|(d, _p)| *d != opposite_direction(g.direction)) // not go back
                            .filter(|(_d, p)| matches!(self.board[*p], 'P' | ' ' | '.' | '$'))
                            .map(|(d, p)| {
                                let (col, row) = index2xy(p);
                                let (tcol, trow) = index2xy(8 * WIDTH + 13);
                                (d, p, tcol.abs_diff(col) + trow.abs_diff(row))
                            })
                            .min_by(|x, y| x.2.cmp(&y.2))
                            .unwrap();
                    }
                }
                GhostState::Outside => {
                    if !slowdown_ghost(g, self.level) {
                        if g.edible_duration > 0 {
                            //flee pacman
                            (g.direction, g.pos, _) = ghost_moves(g.pos)
                                //.filter(|(d, _p)| *d != opposite_direction(g.direction)) // not go back
                                .filter(|(_d, p)| matches!(self.board[*p], 'P' | ' ' | '.' | '$'))
                                .map(|(d, p)| {
                                    let (col, row) = index2xy(p);
                                    let (tcol, trow) = index2xy(self.player.pos);
                                    (d, p, tcol.abs_diff(col) + trow.abs_diff(row))
                                })
                                .max_by(|x, y| x.2.cmp(&y.2))
                                .unwrap();
                        } else if period(self.level, self.timecum) == Period::Chase {
                            (g.direction, g.pos, _) = ghost_moves(g.pos)
                                .filter(|(d, _p)| *d != opposite_direction(g.direction)) // not go back
                                .filter(|(_d, p)| matches!(self.board[*p], 'P' | ' ' | '.' | '$'))
                                .map(|(d, p)| {
                                    let (col, row) = index2xy(p);
                                    let (tcol, trow) = index2xy(chase_target[gidx]);
                                    (d, p, tcol.abs_diff(col) + trow.abs_diff(row))
                                })
                                .min_by(|x, y| x.2.cmp(&y.2))
                                .unwrap();
                        } else {
                            // scatter mode
                            let mut rng = rand::thread_rng();

                            (g.direction, g.pos) = ghost_moves(g.pos)
                                .filter(|(d, _p)| *d != opposite_direction(g.direction)) // not go back
                                .filter(|(_d, p)| matches!(self.board[*p], 'P' | ' ' | '.' | '$'))
                                .choose(&mut rng)
                                .unwrap();
                        }
                    }
                } // Outside
            } // match ghost_state
        })
    }

    fn update(&mut self, dur: u32) {
        self.update_player(dur);
        self.check_player_vs_ghosts();
        self.update_ghosts(dur);
        self.check_player_vs_ghosts();
        self.update_fruit(dur);
    }

    fn next_player_pos(&self, d: Direction) -> usize {
        let col = self.player.pos % WIDTH;
        match d {
            Direction::Right => match col {
                WIDTHM1 => self.player.pos - col, // tunnel
                _ => self.player.pos + 1,
            },
            Direction::Left => match col {
                0 => self.player.pos + (WIDTHM1 - col), // tunnel
                _ => self.player.pos - 1,
            },
            Direction::Down => self.player.pos + WIDTH,
            Direction::Up => self.player.pos - WIDTH,
        }
    }

    fn update_player(&mut self, telaps: u32) {
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

        let ch = self.board[idx];
        match ch {
            'P' | ' ' | '.' | '$' => {
                self.player.pos = idx;
                match ch {
                    '.' => {
                        self.score += 10;
                        self.dots_left -= 1;
                        self.board[idx] = ' ';
                    }
                    'P' => {
                        self.am.play("Audio/eatpill.ogg".to_string());
                        self.board[idx] = ' ';
                        self.ghosts_are_edible(self.pill_duration);
                        self.score += 50;
                        self.next_ghost_score = 200;
                    }
                    '$' => {
                        if self.fruit_duration > 0 {
                            self.am.play("Audio/eatpill.ogg".to_string());
                            let (_ch, bonus) = level2fruit(self.level);
                            self.score += bonus;
                            self.fruit_duration = 0;

                            draw_message(format!("{}", bonus).as_str(), false);
                            thread::sleep(time::Duration::from_millis(150));
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }

        if prev_score < 10000 && self.score >= 10000 && self.lives < MAX_PACMAN_LIVES {
            self.lives += 1;
            self.am.play("Audio/extra lives.ogg".to_string());
        }

        if self.score > self.high_score {
            self.high_score = self.score;
        }
    } // update_player
} // impl Game

fn init_render() {
    crossterm::queue!(
        stdout(),
        style::ResetColor,
        terminal::Clear(terminal::ClearType::All),
        terminal::EnterAlternateScreen,
        cursor::Hide,
        cursor::MoveTo(0, 0)
    )
    .unwrap();

    terminal::enable_raw_mode().ok();
}

fn close_render() {
    crossterm::queue!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        terminal::LeaveAlternateScreen,
        cursor::Show,
        cursor::MoveTo(0, 0)
    )
    .ok();
    terminal::disable_raw_mode().ok();
}

fn draw_end_game() {
    draw_message("GAME  OVER", true);
}

fn draw_message(s: &str, blink: bool) {
    let col: u16 = ((WIDTH - s.len()) / 2).try_into().unwrap();
    let s1 = match blink {
        true => s.bold().slow_blink(),
        false => s.bold(),
    };
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, 14),
        style::PrintStyledContent(s1.bold())
    )
    .ok();
    stdout().flush().ok();
}

fn draw_message_at(pos: usize, s: &str) {
    let (mut col, row) = index2xy(pos);
    if col > WIDTH as u16 - 4 {
        col = WIDTH as u16 - 4;
    }
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, row),
        style::PrintStyledContent(s.bold())
    )
    .ok();
    stdout().flush().ok();
}

fn draw_start_game() {
    draw_message("READY!", false);
    thread::sleep(time::Duration::from_millis(1000));
}

fn centered_x(s: &str) -> u16 {
    let leftedge: u16 = 32;
    let n: u16 = s.len().try_into().unwrap();

    match terminal::size() {
        Ok((cols, _rows)) => (cols - leftedge - n) / 2 + leftedge,
        Err(_) => leftedge,
    }
}

fn another_game() -> bool {
    let s1 = "Another game, squire?";
    let s2 = "Y/N";

    crossterm::queue!(
        stdout(),
        cursor::MoveTo(centered_x(s1), 12),
        style::PrintStyledContent(s1.red()),
        cursor::MoveTo(centered_x(s2), 14),
        style::PrintStyledContent(s2.red()),
    )
    .ok();
    stdout().flush().ok();

    loop {
        match read() {
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('y'),
                ..
            }))
            | Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('Y'),
                ..
            })) => return true,
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('n'),
                ..
            }))
            | Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('N'),
                ..
            })) => return false,
            _ => (),
        }
    }
}

fn pause() -> bool {
    draw_message("PAUSED", false);
    loop {
        if let Ok(Event::Key(KeyEvent {
            code: KeyCode::Char(' '),
            ..
        })) = read()
        {
            return true;
        }
        // match read() {
        //     Ok(Event::Key(KeyEvent {
        //         code: KeyCode::Char(' '),
        //         ..
        //     })) => return true,
        //     _ => (),
        // }
    }
}

fn render_game_info() {
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
    .ok();
}

fn animate_dead_player(game: &Game) {
    let sz_anim = "|Vv_.+*X*+. ";
    for ch in sz_anim.chars() {
        draw_board(game, false);

        let (col, row) = index2xy(game.player.pos);
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(col, row),
            style::PrintStyledContent(ch.bold().yellow()),
        )
        .ok();
        stdout().flush().ok();

        thread::sleep(time::Duration::from_millis(150));
    }
}

fn render_rhs(game: &Game) {
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
    )
    .ok();

    let (ch, _bonus) = level2fruit(game.level);
    draw_message_at(25 * WIDTH - 1, ch);

    let s = vec!['\u{1F642}'; game.lives as usize];
    let s1 = vec![' '; MAX_PACMAN_LIVES as usize - s.len()];
    let s2: String = s.into_iter().chain(s1).collect::<String>();
    draw_message_at(24 * WIDTH, &s2);

    // scroll marquee
    // let (cols, rows) = match terminal::size() {
    //     Ok((cols, rows)) => (cols, rows),
    //     Err(_) => (0, 0), // panic!
    // };

    // let i: u16 = if cols > WIDTH.try_into().unwrap() {
    //     0
    // } else {
    //     WIDTH.try_into().unwrap()
    // };

    // let q: u16 = cols - i;

    // let i1: usize = game.mq_idx % MARQUEE.len();
    // let t: usize = q as usize + game.mq_idx;
    // let i2: usize = t % MARQUEE.len();
    // crossterm::execute!(stdout(), cursor::MoveTo(i, rows - i)).ok();
    // if i1 < i2 {
    //     crossterm::execute!(stdout(), style::PrintStyledContent(MARQUEE[i1..i2].white())).ok();
    // } else {
    //     crossterm::execute!(
    //         stdout(),
    //         style::PrintStyledContent(
    //             format!("{}{}", &MARQUEE[i1..MARQUEE.len() - 1], &MARQUEE[0..i2]).white()
    //         )
    //     )
    //     .ok();
    // }
}

fn draw_board(game: &Game, bold: bool) {
    game.board.iter().enumerate().for_each(|(i, c)| {
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
        )
        .ok();
    });

    // print fruit separately - because not rendered correctly otherwise (is wider than one cell)
    if game.fruit_duration > 0 {
        let (s, _bonus) = level2fruit(game.level);
        game.board
            .iter()
            .enumerate()
            .filter(|(_, &c)| c == '$')
            .for_each(|(i, _)| {
                let (col, row) = index2xy(i);
                crossterm::queue!(stdout(), cursor::MoveTo(col, row), style::Print(s),).ok();
            })
    }
}

fn flash_board(game: &Game) {
    for i in 0..10 {
        draw_board(game, i % 2 == 0);
        stdout().flush().ok();

        thread::sleep(time::Duration::from_millis(300));
    }
}

fn draw_player(game: &Game) {
    let sz_anim = match game.player.last_input_direction {
        Direction::Left => ['}', ')', '>', '-', '>', ')'],
        Direction::Right => ['{', '(', '<', '-', '<', '('],
        Direction::Up => ['V', 'V', 'V', 'V', '|', '|'],
        Direction::Down => ['^', '^', '^', '^', '|', '|'],
    };
    let (col, row) = index2xy(game.player.pos);
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, row),
        style::PrintStyledContent(sz_anim[game.player.anim_frame].bold().yellow()),
    )
    .ok();
}

fn draw_ghosts(game: &Game) {
    game.ghosts.iter().enumerate().for_each(|(i, g)| {
        let s = match g.ghost_state {
            GhostState::Dead => "\u{1F440}",
            _ => {
                if game.board[g.pos] != 'H' && g.edible_duration > 0 {
                    if g.edible_duration < 2000 {
                        //"\u{1F631}".rapid_blink() // looks bad
                        "\u{1F47D}" // alien
                    } else {
                        "\u{1F631}" // Scream
                    }
                } else {
                    match i {
                        0 => "\u{1F47A}", // Goblin
                        1 => "\u{1F479}", // Ogre
                        2 => "\u{1F47B}", // Ghost
                        _ => "\u{1F383}", // Jack-O-Lantern
                    }
                }
            }
        };
        let (col, row) = index2xy(g.pos);
        crossterm::queue!(stdout(), cursor::MoveTo(col, row), style::Print(s),).ok();
    });
}

//  The animated death and flashing screen happen syncronously. To be done
//  correctly, they should be pseudo-event driven like the rest of the program.
fn draw_dynamic(game: &Game) {
    draw_board(game, false);
    draw_player(game);
    draw_ghosts(game);
    render_rhs(game);
    stdout().flush().ok();
}

fn game_loop(game: &mut Game) -> GameState {
    loop {
        let start = Instant::now();

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
                })) => return GameState::UserQuit,
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
                })) => Direction::Left,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Right,
                    ..
                })) => Direction::Right,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Up, ..
                })) => Direction::Up,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    ..
                })) => Direction::Down,
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char(' '),
                    ..
                })) => {
                    pause();
                    game.player.last_input_direction
                }
                _ => game.player.last_input_direction,
            };
        }
        game.mq_idx = (game.mq_idx + 1) % MARQUEE.len(); // scroll marquee

        game.update((Instant::now() - start).as_millis().try_into().unwrap());
        draw_dynamic(game);

        if game.player.dead {
            return GameState::LifeLost;
        }

        match game.dots_left {
            0 => return GameState::SheetComplete,
            74 | 174 => {
                game.fruit_duration = 1000 * (10 + random::<u32>() % 3);
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
    fn play(&mut self, name: String) {
        self.manager
            .play(self.sounds.get(&name).unwrap().clone())
            .ok();
    }
}

fn main_game() {
    let mut game = Game::new();
    render_game_info();
    loop {
        draw_dynamic(&game);
        draw_start_game();
        thread::sleep(time::Duration::from_millis(200));
        match game_loop(&mut game) {
            GameState::UserQuit => return,
            GameState::SheetComplete => {
                game.am.play("Audio/opening_song.ogg".to_string());
                flash_board(&game);
                game.level += 1;
                game.repopulate_board();
                game.ghosts = GHOSTS_INIT;
                game.player = PLAYER_INIT;
                game.timecum = 0;
            }
            GameState::LifeLost => {
                render_rhs(&game);
                game.am.play("Audio/die.ogg".to_string());
                animate_dead_player(&game);
                if game.lives == 0 {
                    return;
                }
                game.lives -= 1;
                thread::sleep(time::Duration::from_millis(100));
                game.ghosts = GHOSTS_INIT;
                game.player = PLAYER_INIT;
            }
        };
    }
}

fn main() {
    init_render();
    while {
        main_game();
        draw_end_game();
        another_game()
    } {}
    close_render();
}
