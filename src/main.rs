use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, poll, read},
    style::{self, Stylize},
    terminal,
};

use rand::random;
use std::collections::HashMap;
use std::io::{self, Write, stdout};
use std::{thread, time};

use kira::{
    manager::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackend},
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
    "############################", //  0
    "#............##............#", //  1
    "#.####.#####.##.#####.####.#", //  2
    "#P####.#####.##.#####.####P#", //  3
    "#..........................#", //  4
    "#.####.##.########.##.####.#", //  5
    "#......##....##....##......#", //  6
    "######.##### ## #####.######", //  7
    "     #.##          ##.#     ", //  8
    "     #.## ###--### ##.#     ", //  9
    "######.## # HHHH # ##.######", // 10
    "      .   # HHHH #   .      ", // 11
    "######.## # HHHH # ##.######", // 12
    "     #.## ######## ##.#     ", // 13
    "     #.##    $     ##.#     ", // 14
    "######.## ######## ##.######", // 15
    "#............##............#", // 16
    "#.####.#####.##.#####.####.#", // 17
    "#P..##................##..P#", // 18
    "###.##.##.########.##.##.###", // 19
    "#......##....##....##......#", // 20
    "#.##########.##.##########.#", // 21
    "#..........................#", // 22
    "############################", // 23
);

fn tunnel(pos: usize) -> bool {
    (11 * WIDTH..=11 * WIDTH + 5).contains(&pos)
        || (11 * WIDTH + 22..=11 * WIDTH + WIDTHM1).contains(&pos)
}

const fn index2xy(i: usize) -> (u16, u16) {
    // crossterm needs u16 coordinates - this version of index2xy is for that
    let (col, row) = index2xy_usize(i);
    (col as u16, row as u16)
}

const fn index2xy_usize(i: usize) -> (usize, usize) {
    (i % WIDTH, i / WIDTH)
}

const fn xy2index(col: usize, row: usize) -> usize {
    row * WIDTH + col
}

static MARQUEE: &str = "Title: A Dialogue Between Plato and Socrates on Pac-Man. \
    Scene: A quiet garden in Athens. Plato and Socrates sit on a stone bench, discussing the nature of games. \
    Socrates: Tell me, Plato, have you observed this peculiar game known as Pac-Man? \
    Plato: I have heard of it, Socrates, though I confess I do not fully grasp its essence. \
    Socrates: It is a game in which a small, ever-hungry being, pursued by ghosts, traverses a maze, consuming pellets for sustenance. \
    Plato: A curious notion! But tell me, Socrates, what wisdom is to be found in such a pursuit? \
    Socrates: Ah, my dear Plato, is it not the case that in life we, too, navigate a labyrinth filled with obstacles, ever striving for fulfillment, yet always pursued by unseen forces? \
    Plato: You suggest that the game is an allegory for the human condition? \
    Socrates: Indeed. Consider the ghosts, are they not akin to our fears and regrets, which chase us through the corridors of existence? Yet, when Pac-Man finds the mighty Power Pellet, he turns upon his pursuers. Is this not a lesson in courage? That with wisdom and preparation, we may face our fears and render them powerless? \
    Plato: A compelling thought, Socrates. Yet, the maze itself, does it not resemble my own theory of forms? For within the cave of the game screen, shadows flicker, but the true reality, the ideal Pac-Man, exists beyond it. \
    Socrates: You imply that what we see on the screen is but an imitation of a higher truth? \
    Plato: Precisely! The game is but a shadow of the true game, an ideal form where every move is perfect, every strategy divine. \
    Socrates: And yet, Plato, if the game is but an imitation, does that make the pursuit meaningless? Or is it, rather, a reflection of the soul's journey, ever striving for perfection but constrained by its mortal form? \
    Plato: I see now, Socrates! Pac-Man is not merely a game, it is philosophy in motion. The wise player, like the philosopher, must understand the patterns of the maze, anticipate the movements of fate, and seize opportunity when it appears. \
    Socrates: You have grasped it well, my friend. But tell me-shall we now play a round and test our understanding in practice? \
    Plato: Only if you promise not to engage me in paradoxes while I concentrate! \
    (They both laugh as they rise, their discourse having brought them to a newfound appreciation of both wisdom and play.) \
    Fin.";

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
    state: GhostState,
    edible_duration: u128,
    direction: Direction,
}

impl Ghost {
    const fn new(pos: usize) -> Self {
        Ghost {
            pos,
            direction: Left,
            edible_duration: 0,
            state: GhostState::Home,
        }
    }

    fn slow(&self, level: u32) -> bool {
        match level {
            0 if tunnel(self.pos) => pct(60),
            0 if self.edible_duration > 0 => pct(60),
            0 => pct(25),
            1..=3 if tunnel(self.pos) => pct(55),
            1..=3 if self.edible_duration > 0 => pct(50),
            1..=3 => pct(15),
            _ if tunnel(self.pos) => pct(50),
            _ if self.edible_duration > 0 => pct(45),
            _ => pct(5),
        }
    }

    fn moves(&self, board: &[char], target: usize) -> (Direction, usize) {
        let (col, row) = index2xy_usize(self.pos);
        let (tcol, trow) = index2xy_usize(target);

        [Right, Left, Down, Up]
            .into_iter()
            .filter_map(|d| {
                let p = match (d, col) {
                    (Right, WIDTHM1) => row * WIDTH, // Tunnel
                    (Right, _) => self.pos + 1,
                    (Left, 0) => row * WIDTH + WIDTH - 1, // Tunnel
                    (Left, _) => self.pos - 1,
                    (Down, _) => self.pos + WIDTH,
                    (Up, _) => self.pos - WIDTH,
                };

                // never go back unless fleeing pacman
                if matches!(board[p], 'P' | ' ' | '.' | '$')
                    && (self.edible_duration > 0 || d != self.direction.opposite())
                {
                    let (ncol, nrow) = index2xy_usize(p);
                    let dst = tcol.abs_diff(ncol) + trow.abs_diff(nrow);
                    Some((dst as isize, d, p))
                } else {
                    None
                }
            })
            .max_by_key(
                |&(dst, _, _)| {
                    if self.edible_duration > 0 { dst } else { -dst }
                },
            )
            .map(|(_, dir, pos)| (dir, pos))
            .unwrap_or((self.direction, self.pos)) // Default to stay in place if no move is possible - never happens
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
        };

        game.repopulate_board();
        game.ghosts = GHOSTS_INIT;
        game
    }

    fn repopulate_board(&mut self) {
        self.board = LEVEL1MAP.chars().collect::<Vec<_>>().try_into().unwrap();
        self.dots_left = self.board.iter().filter(|&c| *c == '.').count() as u32;
        self.dots_left += 2; // +2 pseudo dots for fruit bonuses
    }

    fn ghosts_are_edible(&mut self, duration: u128) {
        for g in self.ghosts.iter_mut() {
            if g.state == GhostState::Outside {
                g.edible_duration += duration
            }
        }
    }

    fn check_player_vs_ghosts(&mut self) -> io::Result<()> {
        for g in self.ghosts.iter_mut() {
            if g.state != GhostState::Dead && g.pos == self.player.pos {
                if g.edible_duration == 0 {
                    self.player.dead = true;
                } else {
                    self.am.play("Audio/eatghost.ogg".to_string())?;
                    self.score += self.next_ghost_score;

                    draw_message_at(g.pos, &format!("{}", self.next_ghost_score))?;
                    thread::sleep(time::Duration::from_millis(150));

                    self.next_ghost_score *= 2;
                    g.state = GhostState::Dead;
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
        const SCATTER_TARGET: [usize; 4] = [
            xy2index(2, 0),
            xy2index(WIDTH - 3, 0),
            xy2index(0, 24),
            xy2index(WIDTH - 1, 24),
        ];
        // Calc chase mode target pos for Pinky, Blinky, Inky & Clyde
        let mut chase_target: [usize; 4] = [self.player.pos; 4];
        // Pinky - target pacman pos
        // Blinky - target 4 squares away from pacman
        let (pcol, prow) = index2xy_usize(self.player.pos);
        chase_target[1] = match self.player.moving {
            Left => prow * WIDTH + pcol.saturating_sub(4),
            Right => prow * WIDTH + std::cmp::min(pcol + 4, WIDTHM1),
            Up => prow.saturating_sub(4) * WIDTH + pcol,
            Down => self.player.pos + 4 * WIDTH,
        };
        // Inky - target average of pacman pos and Blinky
        let (bcol, brow) = index2xy_usize(self.ghosts[1].pos);
        let (tcol, trow) = ((pcol + bcol) / 2, (prow + brow) / 2);

        chase_target[2] = trow * WIDTH + tcol;

        // Clyde - target pacman if less than 8 squares away - otherwise target corner
        let (bcol, brow) = index2xy_usize(self.ghosts[3].pos);
        let dist = bcol.abs_diff(pcol).pow(2) + brow.abs_diff(prow).pow(2);
        if dist >= 64 {
            chase_target[3] = SCATTER_TARGET[3]
        }

        for (gidx, g) in self.ghosts.iter_mut().enumerate() {
            g.edible_duration = g.edible_duration.saturating_sub(telaps);
            (g.direction, g.pos) = match g.state {
                GhostState::Home => {
                    let a = [g.pos - 1, g.pos + 1, g.pos - WIDTH, g.pos + WIDTH];
                    let idx = a[random::<usize>() % a.len()];
                    match self.board[idx] {
                        'H' => (Left, idx),
                        '-' => {
                            g.state = GhostState::Gateway;
                            (Left, idx)
                        }
                        _ => (g.direction, g.pos),
                    }
                }
                GhostState::Gateway => {
                    g.state = GhostState::Outside;
                    match random::<u8>() % 2 {
                        0 => (Left, g.pos - WIDTH),
                        _ => (Right, g.pos - WIDTH),
                    }
                }

                GhostState::Dead => {
                    // if at house gate - go in
                    if g.pos == 8 * WIDTH + 13 || g.pos == 8 * WIDTH + 14 {
                        (Down, g.pos + WIDTH)
                    } else if g.pos == 9 * WIDTH + 13 || g.pos == 9 * WIDTH + 14 {
                        g.state = GhostState::Home;
                        (g.direction, g.pos + WIDTH)
                    } else {
                        g.moves(&self.board, 8 * WIDTH + 13)
                    }
                }
                GhostState::Outside => {
                    if g.slow(self.level) {
                        continue;
                    }
                    match (g.edible_duration > 0, period(self.level, self.timecum)) {
                        (true, _) => g.moves(&self.board, self.player.pos),
                        (false, Period::Chase) => g.moves(&self.board, chase_target[gidx]),
                        (false, Period::Scatter) => g.moves(&self.board, SCATTER_TARGET[gidx]),
                    }
                }
            } // match ghost_state
        }
    }

    fn update(&mut self, dur: u128) -> io::Result<()> {
        self.update_player(dur)?;
        self.check_player_vs_ghosts()?;
        self.update_ghosts(dur);
        self.check_player_vs_ghosts()?;
        self.update_fruit(dur);
        Ok(())
    }

    fn next_player_pos(&self, direction: Direction) -> usize {
        let col = self.player.pos % WIDTH;
        match direction {
            Right if col == WIDTHM1 => self.player.pos - col, // tunnel
            Right => self.player.pos + 1,
            Left if col == 0 => self.player.pos + WIDTHM1 - col, // tunnel
            Left => self.player.pos - 1,
            Down => self.player.pos + WIDTH,
            Up => self.player.pos - WIDTH,
        }
    }

    fn move_player(&mut self, idx: usize) -> io::Result<bool> {
        // move may not be valid - return true if valid
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
            ' ' | '$' => (),
            _ => return Ok(false),
        }
        self.player.pos = idx;
        Ok(true)
    }

    fn update_player(&mut self, telaps: u128) -> io::Result<()> {
        self.player.timecum += telaps;
        while self.player.timecum > 100 {
            self.player.timecum -= 100;
            self.player.anim_frame = (self.player.anim_frame + 1) % 6;
        }

        let prev_score = self.score;

        // Try moving in input direction, then fallback to current movement
        let idx = self.next_player_pos(self.player.last_input_direction);
        match self.move_player(idx)? {
            true => self.player.moving = self.player.last_input_direction,
            false => {
                let idx = self.next_player_pos(self.player.moving);
                if !self.move_player(idx)? {
                    return Ok(());
                }
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
                code: KeyCode::Char('y' | 'Y'),
                ..
            }) => return Ok(true),
            Event::Key(KeyEvent {
                code: KeyCode::Char('n' | 'N'),
                ..
            }) => return Ok(false),
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
    let s2 = "Rusty Edition 2025 ";

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
    for ch in "|Vv_.+*X*+. ".chars() {
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

    let marquee_x = 0; // start column
    let q: u16 = cols - marquee_x;
    let i1: usize = game.mq_idx % MARQUEE.len();
    let t: usize = q as usize + game.mq_idx;
    let i2: usize = t % MARQUEE.len();
    crossterm::execute!(stdout(), cursor::MoveTo(marquee_x, rows - 1))?;
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
    let ch = match game.player.last_input_direction {
        Left => ['}', ')', '>', '-', '>', ')'],
        Right => ['{', '(', '<', '-', '<', '('],
        Up => ['V', 'V', 'V', 'V', '|', '|'],
        Down => ['^', '^', '^', '^', '|', '|'],
    }[game.player.anim_frame];
    let (col, row) = index2xy(game.player.pos);
    crossterm::queue!(
        stdout(),
        cursor::MoveTo(col, row),
        style::PrintStyledContent(ch.bold().yellow()),
    )
}

fn draw_ghosts(game: &Game) -> io::Result<()> {
    //"\u{1F631}".rapid_blink() // looks bad
    for (i, g) in game.ghosts.iter().enumerate() {
        let s = match (g.state, game.board[g.pos] != 'H', i) {
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
        if game.ghosts.iter().any(|g| g.edible_duration > 0) {
            delta -= 20; // faster if power pill eaten
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
