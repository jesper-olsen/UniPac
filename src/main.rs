use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    style::{self, Stylize},
    terminal, //QueueableCommand, Result,
};

// TODO - speed function of level

use std::io::{stdout, Write};

use rand::random;
use std::collections::HashMap;
use std::time::Instant;
use std::{thread, time};

use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
};

const MAX_PACMAN_LIVES: u32 = 6;
const WIDTH: usize = 28;

//const SZ_SPECIAL0: [&str; 4] = ["$", "@", "%", "!"];
const SZ_SPECIAL: [&str; 6] = [
    "\u{1F352}", // cherries
    "\u{1F353}", // strawberry
    "\u{1F34E}", // red apple
    "\u{1F351}", // peach
    "\u{1F514}", // bell
    "\u{1F511}", // key
];

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

fn index2xy(i: usize) -> (u16, u16) {
    (
        (i % WIDTH).try_into().unwrap(),
        (i / WIDTH).try_into().unwrap(),
    )
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
    Shuffle,
    Gateway,
    Outside,
}

struct Ghost {
    pos: usize,
    active: bool,
    ghost_state: GhostState,
    edible_duration: u32,
    direction: Direction,
}

impl Ghost {
    fn activate(&mut self) {
        self.pos = 10 * WIDTH + 12;
        self.direction = Direction::Left;
        self.active = true;
        self.edible_duration = 0;
        self.ghost_state = GhostState::Shuffle;
    }
}

struct Player {
    pos: usize,
    dead: bool,
    score: u32,
    last_input_direction: Direction,
    moving: Direction,
    anim_frame: usize,
    next_ghost_score: u32,
    timecum: u32,
}

impl Player {
    pub fn new() -> Self {
        Player {
            dead: false,
            pos: 18 * WIDTH + 14,
            last_input_direction: Direction::Left,
            moving: Direction::Left,
            timecum: 0,
            anim_frame: 0,
            score: 0,
            next_ghost_score: 0,
        }
    }
}

struct Game {
    board: Vec<char>,
    mq_idx: usize,
    timecum: u32,
    dots_left: u32,
    high_score: u32,
    lives: u32,
    player: Player,
    level: u32,
    ghosts: Vec<Ghost>,
    pill_duration: u32,
    special_idx: usize,
    special_duration: u32,
    time_before_special: u32,
    special_pos: usize,
    am: AM,
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
            "Audio/opening_song.ogg",
        ]
        .iter()
        {
            let snd = StaticSoundData::from_file(s, StaticSoundSettings::default())
                .expect("Failed to load sound");
            //manager.play(snd.clone());
            sounds.insert(s.to_string(), snd);
        }

        let mut game = Game {
            timecum: 0,
            mq_idx: 0,
            ghosts: vec![],
            pill_duration: 6000,
            special_idx: 0,
            special_pos: 14 * WIDTH + 14,
            level: 0,
            board: LEVEL1MAP.to_string().chars().collect(),
            dots_left: 0,
            high_score: 9710,
            lives: 3,
            player: Player::new(),
            time_before_special: 1000 * (10 + (random::<u32>() % 4) * 5),
            special_duration: 0,
            am: AM { manager, sounds },
        };

        game.initialise();
        game
    }
    fn repopulate_board(&mut self) {
        self.board = LEVEL1MAP.to_string().chars().collect();
        self.dots_left = self
            .board
            .iter()
            .filter(|&c| *c == '.')
            .count()
            .try_into()
            .unwrap();
    }

    fn initialise_special(&mut self) {
        self.timecum = 0;
        self.special_idx = self.level.try_into().unwrap();
        self.special_pos = 14 * WIDTH + 14;
        self.respawn_special();
    }

    fn initialise(&mut self) {
        self.repopulate_board();
        self.initialise_ghosts();
        self.initialise_special();
    }

    fn ghosts_are_edible(&mut self, duration: u32) {
        self.ghosts
            .iter_mut()
            .filter(|g| g.active && g.ghost_state == GhostState::Outside)
            .for_each(|g| {
                g.edible_duration += duration;
            })
    }

    fn respawn_special(&mut self) {
        self.time_before_special = 1000 * (10 + (random::<u32>() % 4) * (5 + self.level));
        self.special_duration = 0;
    }

    fn check_player_vs_ghosts(&mut self) {
        self.ghosts
            .iter_mut()
            .filter(|g| g.active && g.pos == self.player.pos)
            .for_each(|g| {
                if g.edible_duration == 0 {
                    self.player.dead = true;
                } else {
                    self.am.play("Audio/eatghost.ogg".to_string());
                    self.player.score += self.player.next_ghost_score;
                    self.player.next_ghost_score *= 2;
                    // todo: trace eyes back to home
                    g.active = false;
                }
            })
    }

    fn update_special(&mut self, telaps: u32) {
        self.timecum += telaps;
        if self.timecum > 500 {
            self.timecum = 0;
        }

        if self.special_duration > 0 {
            if self.special_duration < telaps {
                self.special_duration = 0;
                self.time_before_special = 1000 * (10 + (5 + self.level) * (random::<u32>() % 4));
            } else {
                self.special_duration -= telaps;
            }
        } else if self.time_before_special <= telaps {
            self.time_before_special = 0;
            self.special_duration = 1000 * (10 + random::<u32>() % 3);
        } else {
            self.time_before_special -= telaps;
        }
    }

    fn reinitialise_player(&mut self) {
        self.player.dead = false;
        self.player.pos = 18 * WIDTH + 14;
        self.player.last_input_direction = Direction::Left;
        self.player.moving = Direction::Left;
        self.player.timecum = 0;
        self.player.anim_frame = 0;
    }

    fn update_ghosts(&mut self, telaps: u32) {
        self.ghosts.iter_mut().for_each(|g| {
            if g.edible_duration < telaps {
                g.edible_duration = 0;
            } else {
                g.edible_duration -= telaps;
            }
            if !g.active {
                if random::<u8>() % 30 < 2 {
                    g.activate();
                }
            } else {
                match g.ghost_state {
                    GhostState::Shuffle => {
                        let idx = g.pos - WIDTH;
                        if random::<u8>() % 2 == 0 && self.board[idx] == '-' {
                            g.ghost_state = GhostState::Gateway;
                            g.pos = idx;
                        } else {
                            match random::<u8>() % 3 {
                                0 => {
                                    let idx = g.pos - 1;
                                    if self.board[idx] == 'H' {
                                        g.pos = idx;
                                    }
                                }
                                1 => {
                                    let idx = g.pos + 1;
                                    if self.board[idx] == 'H' {
                                        g.pos = idx;
                                    }
                                }
                                _ => (),
                            }
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
                    GhostState::Outside => {
                        let m: HashMap<Direction, usize> = [
                            (Direction::Right, g.pos + 1),
                            (Direction::Left, g.pos - 1),
                            (Direction::Down, g.pos + WIDTH),
                            (Direction::Up, g.pos - WIDTH),
                        ]
                        .iter()
                        .map(|(d, p)| {
                            let col = *p % WIDTH;
                            let row = *p / WIDTH;
                            if col == 0 {
                                // tunnel
                                (*d, row * WIDTH + WIDTH - 1)
                            } else if col == WIDTH - 1 {
                                // tunnel
                                (*d, row * WIDTH)
                            } else {
                                (*d, *p)
                            }
                        })
                        .filter(|(_, pos)| matches!(self.board[*pos], 'P' | ' ' | '.' | '$'))
                        .collect();

                        if random::<u8>() % 34 == 0 && g.pos != self.player.pos {
                            // random direction
                            let keys: Vec<&Direction> = m.keys().collect();
                            let key = keys[random::<usize>() % keys.len()];
                            g.pos = m[key];
                            g.direction = *key;
                        } else if m.contains_key(&g.direction) {
                            // same direction
                            g.pos = m[&g.direction];
                        } else {
                            // Have to change direction
                            let l = match g.direction {
                                Direction::Left | Direction::Right => {
                                    [Direction::Up, Direction::Down]
                                }
                                Direction::Up | Direction::Down => {
                                    [Direction::Left, Direction::Right]
                                }
                            };
                            let idx = random::<usize>() % 2;
                            if m.contains_key(&l[idx]) {
                                g.direction = l[idx];
                            } else {
                                g.direction = l[(idx + 1) % 2];
                            }
                            g.pos = m[&g.direction];
                        }
                    } // Outside
                } // match ghost_state
            }
        })
    }

    fn update(&mut self, dur: u32) {
        self.update_player(dur);
        self.check_player_vs_ghosts();
        self.update_ghosts(dur);
        self.check_player_vs_ghosts();
        self.update_special(dur);
    }

    fn next_player_pos(&self, d: Direction) -> usize {
        const WIDTHM1: usize = WIDTH - 1;
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

        let prev_score = self.player.score;

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
                        self.player.score += 10;
                        self.dots_left -= 1;
                        self.board[idx] = ' ';
                    }
                    'P' => {
                        self.am.play("Audio/eatpill.ogg".to_string());
                        self.board[idx] = ' ';
                        self.ghosts_are_edible(self.pill_duration);
                        self.player.score += 50;
                        self.player.next_ghost_score = 200;
                    }
                    '$' => {
                        if self.special_duration > 0 {
                            self.am.play("Audio/eatpill.ogg".to_string());
                            self.player.score += 100 + self.special_idx as u32 * 100;
                            self.special_idx = (self.special_idx + 1) % SZ_SPECIAL.len();
                            self.respawn_special();
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }

        if prev_score < 10000 && self.player.score >= 10000 {
            if self.lives < MAX_PACMAN_LIVES {
                self.lives += 1;
            }
        }

        if self.player.score > self.high_score {
            self.high_score = self.player.score;
        }
    } // update_player

    fn initialise_ghosts(&mut self) {
        const MAX_GHOSTS: usize = 4;
        self.ghosts = vec![];
        for i in 0..MAX_GHOSTS {
            self.ghosts.push(Ghost {
                active: i < MAX_GHOSTS - 1,
                pos: 10 * WIDTH + 12 + i * 2,
                direction: Direction::Left,
                edible_duration: 0,
                ghost_state: GhostState::Shuffle,
            });
        }
    }
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
    let s = "GAME  OVER".bold().slow_blink();
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(9, 14),
        style::PrintStyledContent(s)
    )
    .ok();
    stdout().flush().ok();
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
fn render_game_info() {
    let s1: &str = "UniPac - Unicode-powered Pacman";
    let s2 = "Rusty Edition 2023 ";

    /*
        ncurses::clear();
        ncurses::attron(ncurses::COLOR_PAIR(PC_PILL));
        ncurses::mvprintw(2, centered_x(s1), s1);
        ncurses::attroff(ncurses::COLOR_PAIR(PC_PILL));
        ncurses::attron(ncurses::COLOR_PAIR(PC_PACMAN));
        ncurses::mvprintw(3, centered_x(s2), s2);
        ncurses::attroff(ncurses::COLOR_PAIR(PC_PACMAN));
    */

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
    let pacimg = ["/-\\", "|'<", "\\_/", "   ", "   ", "   "];
    // need to remove the old pacman character in some cases
    for i in 0..MAX_PACMAN_LIVES {
        for j in 0..3 {
            let q = if game.lives > i { 0 } else { 3 };
            crossterm::queue!(
                stdout(),
                cursor::MoveTo(
                    (i * 5 + 30).try_into().unwrap(),
                    (16 + j).try_into().unwrap()
                ),
                style::PrintStyledContent(pacimg[(j + q) as usize].bold().yellow()),
            )
            .ok();
        }
    }

    let i = centered_x("Score : 123456"); /* get a pos base on av score digits */
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(i.try_into().unwrap(), 5.try_into().unwrap()),
        style::PrintStyledContent(format!("Score : {}", game.player.score).bold().white()),
        cursor::MoveTo(i.try_into().unwrap(), 6.try_into().unwrap()),
        style::PrintStyledContent(format!("High  : {}", game.high_score).bold().white()),
        cursor::MoveTo(i.try_into().unwrap(), 8.try_into().unwrap()),
        style::PrintStyledContent(format!("Level  : {}", game.level).bold().white()),
    )
    .ok();

    let (cols, rows) = match terminal::size() {
        Ok((cols, rows)) => (cols, rows),
        Err(_) => (0, 0), // panic!
    };

    let i: u16 = if cols > WIDTH.try_into().unwrap() {
        0
    } else {
        WIDTH.try_into().unwrap()
    };

    //let q: usize = ncurses::COLS() as usize - i;
    let q: u16 = cols as u16 - i;

    let i1: usize = game.mq_idx % MARQUEE.len();
    let t: usize = q as usize + game.mq_idx;
    let i2: usize = t % MARQUEE.len();
    if i1 < i2 {
        // ncurses::mvprintw(
        //     ncurses::LINES() - 1,
        //     i.try_into().unwrap(),
        //     &MARQUEE[i1..i2],
        // );
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(i.try_into().unwrap(), rows - 1),
            style::PrintStyledContent(MARQUEE[i1..i2].white())
        )
        .ok();
    } else {
        // ncurses::mvprintw(
        //     ncurses::LINES() - 1,
        //     i.try_into().unwrap(),
        //     format!("{}{}", &MARQUEE[i1..MARQUEE.len() - 1], &MARQUEE[0..i2]).as_str(),
        // );
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(i.try_into().unwrap(), rows - 1),
            style::PrintStyledContent(
                format!("{}{}", &MARQUEE[i1..MARQUEE.len() - 1], &MARQUEE[0..i2]).white()
            ),
        )
        .ok();
    }
}

fn draw_board(game: &Game, bold: bool) {
    game.board.iter().enumerate().for_each(|(i, c)| {
        let mut ch = match *c {
            '#' => "#".blue(),
            '.' => ".".white(),
            // '.' => ".".cyan(),
            //'P' => "\u{1F36A}".slow_blink(), // cookie - too wide
            'P' => "*".slow_blink(),
            _ => " ".white(),
        };
        if bold {
            ch = ch.bold();
        }
        let (col, row) = index2xy(i);
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(col, row),
            style::PrintStyledContent(ch),
        )
        .ok();
    });

    // print separately - because not styled
    if game.special_duration > 0 {
        game.board
            .iter()
            .enumerate()
            .filter(|(_, &c)| c == '$')
            .for_each(|(i, _)| {
                let (col, row) = index2xy(i);
                crossterm::queue!(
                    stdout(),
                    cursor::MoveTo(col, row),
                    style::Print(SZ_SPECIAL[game.special_idx as usize]),
                )
                .ok();
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
    game.ghosts
        .iter()
        .enumerate()
        .filter(|(_, g)| g.active)
        .for_each(|(i, g)| {
            let s = if g.edible_duration > 0 {
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
            };
            let (col, row) = index2xy(g.pos);
            crossterm::queue!(stdout(), cursor::MoveTo(col, row), style::Print(s),).ok();
        });
}

//  The animated death and flashing screen happen syncronously. To be done
//  correctly, they should be pseudo-event driven like the rest of the program.
fn draw_dynamic(game: &Game) {
    draw_board(game, false);
    //draw_special(game);
    draw_player(game);
    draw_ghosts(game);
    render_rhs(game);
    stdout().flush().ok();
}

fn game_loop(game: &mut Game) -> GameState {
    loop {
        let start = Instant::now();
        thread::sleep(time::Duration::from_millis(100));

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
                _ => game.player.last_input_direction,
            };
        }
        game.mq_idx = (game.mq_idx + 1) % MARQUEE.len(); // scroll marquee

        game.update((Instant::now() - start).as_millis().try_into().unwrap());
        draw_dynamic(game);

        if game.dots_left == 0 {
            return GameState::SheetComplete;
        } else if game.player.dead {
            return GameState::LifeLost;
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
        thread::sleep(time::Duration::from_millis(200));
        match game_loop(&mut game) {
            GameState::UserQuit => return,
            GameState::SheetComplete => {
                game.am.play("Audio/opening_song.ogg".to_string());
                flash_board(&game);
                game.level += 1;
                game.repopulate_board();
                game.initialise_ghosts();
                game.reinitialise_player();
                game.initialise_special();
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
                game.initialise_ghosts();
                game.reinitialise_player();
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
