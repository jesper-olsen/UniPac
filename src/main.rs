use crossterm::{
    cursor,
    event::{Event, KeyCode, poll, read},
    style::{self, Stylize},
    terminal,
};

use rand::random;
use std::io::{self, Write, stdout};
use std::{path, thread, time};

use kira::{
    manager::{AudioManager, AudioManagerSettings, backend::cpal::CpalBackend},
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
};

mod board;
mod maze;
use board::{Board, Direction, Direction::*, Position, Square};

const MAX_PACMAN_LIVES: u32 = 6;
fn pct(n: u8) -> bool {
    random::<u8>() % 100 < n
}

// return: fruit symbol & bonus value
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

#[derive(Clone, Copy)]
struct Ghost {
    pos: Position,
    state: GhostState,
    edible_duration: u128,
    direction: Direction,
}

impl Ghost {
    const fn new(pos: Position) -> Self {
        Ghost {
            pos,
            direction: Left,
            edible_duration: 0,
            state: GhostState::Home,
        }
    }

    fn slow(&self, level: u32, in_tunnel: bool) -> bool {
        match level {
            0 if in_tunnel => pct(60),
            0 if self.edible_duration > 0 => pct(60),
            0 => pct(25),
            1..=3 if in_tunnel => pct(55),
            1..=3 if self.edible_duration > 0 => pct(50),
            1..=3 => pct(15),
            _ if in_tunnel => pct(50),
            _ if self.edible_duration > 0 => pct(45),
            _ => pct(5),
        }
    }

    fn moves(&self, board: &Board, target: Position) -> (Direction, Position) {
        [Right, Left, Down, Up]
            .into_iter()
            .filter_map(|d| {
                let p = self.pos.go(d);

                // never go back unless fleeing pacman
                if matches!(
                    board[p],
                    Square::Pill
                        | Square::Empty
                        | Square::Dot
                        | Square::Fruit
                        | Square::Tunnel
                        | Square::Start
                ) && (self.edible_duration > 0 || d != self.direction.opposite())
                {
                    Some((target.dist_city(p) as isize, d, p))
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
    pos: Position,
    dead: bool,
    last_input_direction: Direction,
    moving: Direction,
    anim_frame: usize,
    timecum: u128, // for animation
}

impl Player {
    fn new(board: &Board) -> Player {
        Player {
            pos: board.pacman_start,
            dead: false,
            last_input_direction: Left,
            moving: Left,
            anim_frame: 0,
            timecum: 0,
        }
    }
}

struct Game {
    board: Board,
    mq_idx: usize,
    timecum: u128, // time is divided into Chase/Scatter Periods
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

#[derive(Copy, Clone, PartialEq)]
enum Period {
    Scatter,
    Chase,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum Sound {
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

impl Game {
    fn new() -> Self {
        let manager = AudioManager::<CpalBackend>::new(AudioManagerSettings::default())
            .expect("Failed to create AM");

        let sounds = AUDIO_FILES.map(|audio_file| {
            let path = path::Path::new(AUDIO_DIR).join(audio_file);
            StaticSoundData::from_file(&path, StaticSoundSettings::default())
                .unwrap_or_else(|e| panic!("Failed to load sound: {path:?}: {e}"))
        });

        let level = 0u32;
        let board = Board::new(level);
        let player = Player::new(&board);
        let mut game = Game {
            timecum: 0,
            mq_idx: 0,
            ghosts: [Ghost::new(Position::from_xy(0, 0)); 4],
            pill_duration: 6000,
            level,
            board,
            dots_left: 0,
            high_score: 9710,
            lives: 3,
            player,
            fruit_duration: 0,
            next_ghost_score: 0,
            score: 0,
            am: AM { manager, sounds },
        };
        game.reset_ghosts();
        game.repopulate_board();
        game
    }

    fn pause(&self) -> io::Result<()> {
        {
            let mut w = io::BufWriter::new(stdout());
            self.draw_message(&mut w, "PAUSED", false)?;
        }
        loop {
            if let Ok(Event::Key(key_event)) = read() {
                // Filter out Release/Repeat events for Windows compatibility
                if key_event.kind == crossterm::event::KeyEventKind::Press
                    && key_event.code == KeyCode::Char(' ')
                {
                    return Ok(());
                }
            }
        }
    }

    fn draw_message<W: Write>(&self, w: &mut W, s: &str, blink: bool) -> io::Result<()> {
        let col: u16 = ((self.board.width - s.len()) / 2).try_into().unwrap();
        let s1 = match blink {
            true => s.bold().slow_blink(),
            false => s.bold(),
        };
        crossterm::queue!(
            w,
            cursor::MoveTo(col, self.board.fruit.row() as u16),
            style::PrintStyledContent(s1.bold())
        )?;
        stdout().flush()
    }

    fn draw_message_at<W: Write>(&self, w: &mut W, pos: Position, s: &str) -> io::Result<()> {
        let (col, row) = (
            std::cmp::min(pos.col(), self.board.width - 4) as u16,
            pos.row() as u16,
        );
        crossterm::queue!(
            w,
            cursor::MoveTo(col, row),
            style::PrintStyledContent(s.bold())
        )?;
        stdout().flush()
    }

    fn reset_ghosts(&mut self) {
        self.ghosts = [
            Ghost::new(self.board.ghost_start[0]),
            Ghost::new(self.board.ghost_start[1]),
            Ghost::new(self.board.ghost_start[2]),
            Ghost::new(self.board.ghost_start[3]),
        ];
    }

    fn period(&self) -> Period {
        match self.timecum {
            0..=6999 => Period::Scatter,
            7000..=26999 => Period::Chase,
            27000..=33999 => Period::Scatter,
            34000..=53999 => Period::Chase,
            54000..=58999 => Period::Scatter,
            59000..=78999 if self.level == 0 => Period::Chase,
            79000..=83999 if self.level == 0 => Period::Scatter,
            _ => Period::Chase,
        }
    }

    fn repopulate_board(&mut self) {
        self.board = Board::new(self.level);
        self.dots_left = self.board.dots() as u32;
        self.dots_left += 2; // +2 pseudo dots for fruit bonuses
    }

    fn ghosts_are_edible(&mut self, duration: u128) {
        for g in self.ghosts.iter_mut() {
            if matches!(g.state, GhostState::Outside | GhostState::Gateway) {
            //if g.state == GhostState::Outside {
                g.edible_duration += duration
            }
        }
    }

    fn check_player_vs_ghosts(&mut self) -> io::Result<()> {
        for gidx in 0..self.ghosts.len() {
            let g = &mut self.ghosts[gidx];
            if g.state != GhostState::Dead && g.pos == self.player.pos {
                if g.edible_duration == 0 {
                    self.player.dead = true;
                    break;
                } else {
                    let score = self.next_ghost_score;
                    self.score += score;
                    self.next_ghost_score *= 2;
                    g.state = GhostState::Dead;
                    g.edible_duration = 0;
                    self.am.play(Sound::EatGhost)?;
                    {
                        let mut w = io::BufWriter::new(stdout());
                        self.draw_message_at(&mut w, self.player.pos, &format!("{score}"))?;
                    }
                    thread::sleep(time::Duration::from_millis(150));
                }
            }
        }
        Ok(())
    }

    fn update_fruit(&mut self, telaps: u128) {
        self.timecum += telaps;
        self.fruit_duration = self.fruit_duration.saturating_sub(telaps);
    }

    fn update_ghosts(&mut self, telaps: u128) {
        let scatter_target: [Position; 4] = [
            Position::from_xy(2, 0),
            Position::from_xy(self.board.width - 3, 0),
            Position::from_xy(0, 24),
            Position::from_xy(self.board.width - 1, self.board.height),
        ];
        // Calc chase mode target pos for Pinky, Blinky, Inky & Clyde
        let mut chase_target: [Position; 4] = [self.player.pos; 4];
        // Pinky - target pacman
        // Blinky - target 4 squares away from pacman
        let (col, row) = (self.player.pos.col(), self.player.pos.row());
        chase_target[1] = match self.player.moving {
            Left => Position::from_xy(col.saturating_sub(4), row),
            Right => Position::from_xy(std::cmp::min(col + 4, self.board.width - 1), row),
            Up => Position::from_xy(col, row.saturating_sub(4)),
            Down => Position::from_xy(col, row + 4),
        };

        // Inky - target average of pacman pos and Blinky
        chase_target[2] = self.player.pos.average(self.ghosts[1].pos);

        // Clyde - target pacman if less than 8 squares away - otherwise target corner
        if self.player.pos.dist_sqr(self.ghosts[3].pos) >= 64 {
            chase_target[3] = scatter_target[3]
        }

        let current_period = self.period();
        for (gidx, g) in self.ghosts.iter_mut().enumerate() {
            g.edible_duration = g.edible_duration.saturating_sub(telaps);
            (g.direction, g.pos) = match g.state {
                GhostState::Home => {
                    let pos = g.pos.go([Left, Right, Up, Down][random::<usize>() % 4]);
                    match self.board[pos] {
                        Square::House => (Left, pos),
                        Square::Gate => {
                            g.state = GhostState::Gateway;
                            (Left, pos)
                        }
                        _ => (g.direction, g.pos),
                    }
                }
                GhostState::Gateway => {
                    g.state = GhostState::Outside;
                    match random::<u8>() % 2 {
                        0 => (Left, g.pos.go(Up)),
                        _ => (Right, g.pos.go(Up)),
                    }
                }
                GhostState::Dead => {
                    if g.pos == self.board.gate1 || g.pos == self.board.gate2 {
                        g.state = GhostState::Home;
                        (g.direction, g.pos.go(Down))
                    } else if g.pos == self.board.front_of_gate1
                        || g.pos == self.board.front_of_gate2
                    {
                        (Down, g.pos.go(Down))
                    } else {
                        g.moves(&self.board, self.board.front_of_gate1) // go home
                    }
                }
                GhostState::Outside => {
                    if g.slow(self.level, self.board[g.pos] == Square::Tunnel) {
                        continue;
                    }
                    match (g.edible_duration > 0, current_period) {
                        (true, _) => g.moves(&self.board, self.player.pos),
                        (false, Period::Chase) => g.moves(&self.board, chase_target[gidx]),
                        (false, Period::Scatter) => g.moves(&self.board, scatter_target[gidx]),
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

    fn move_player(&mut self, pos: Position) -> io::Result<bool> {
        // move may not be valid - return true if valid
        match self.board[pos] {
            Square::Dot => {
                self.score += 10;
                self.dots_left -= 1;
                self.board[pos] = Square::Empty;
            }
            Square::Pill => {
                self.am.play(Sound::EatPill)?;
                self.board[pos] = Square::Empty;
                self.ghosts_are_edible(self.pill_duration);
                self.score += 50;
                self.next_ghost_score = 200;
            }
            Square::Fruit if self.fruit_duration > 0 => {
                self.am.play(Sound::EatPill)?;
                let bonus = level2bonus(self.level).1;
                self.score += bonus;
                self.fruit_duration = 0;

                {
                    let mut w = io::BufWriter::new(stdout());
                    self.draw_message(&mut w, &format!("{bonus}"), false)?;
                }
                thread::sleep(time::Duration::from_millis(150));
            }
            Square::Empty | Square::Fruit | Square::Tunnel | Square::Start => (),
            Square::Wall | Square::Gate | Square::House => return Ok(false),
        }
        self.player.pos = pos;
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
        match self.move_player(self.player.pos.go(self.player.last_input_direction))? {
            true => self.player.moving = self.player.last_input_direction,
            false => {
                if !self.move_player(self.player.pos.go(self.player.moving))? {
                    return Ok(());
                }
            }
        }

        if prev_score < 10000 && self.score >= 10000 && self.lives < MAX_PACMAN_LIVES {
            self.lives += 1;
            self.am.play(Sound::ExtraLives)?;
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

fn centered_x(s: &str) -> u16 {
    let leftedge: u16 = 32;
    let Ok((cols, _rows)) = terminal::size() else {
        return leftedge;
    };
    let n: u16 = s.len().try_into().unwrap();
    let offset = cols.saturating_sub(leftedge).saturating_sub(n) / 2;
    offset + leftedge
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
        if let Ok(Event::Key(key_event)) = read() {
            // Filter out Release/Repeat events for Windows compatibility
            if key_event.kind == crossterm::event::KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Char('y' | 'Y') => return Ok(true),
                    KeyCode::Char('n' | 'N') => return Ok(false),
                    _ => (),
                }
            }
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
        let mut w = io::BufWriter::new(stdout());
        draw_board(&mut w, game, false)?;
        crossterm::queue!(
            w,
            cursor::MoveTo(game.player.pos.col() as u16, game.player.pos.row() as u16),
            style::PrintStyledContent(ch.bold().yellow()),
        )?;
        w.flush()?;

        thread::sleep(time::Duration::from_millis(150));
    }
    Ok(())
}

fn render_rhs<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
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
    //         )?;
    //     }
    // }

    let s = if game.period() == Period::Chase {
        "\u{1F4A1}" // light bulb
    } else {
        "  "
    };
    crossterm::queue!(
        w,
        cursor::MoveTo(game.board.width as u16 + 2, game.board.height as u16 - 1),
        style::Print(s)
    )?;

    let i = centered_x("Score : 123456"); // get a pos base on av score digits
    crossterm::queue!(
        w,
        cursor::MoveTo(i, 5),
        style::PrintStyledContent(format!("Maze   : {}", game.board.maze_name).bold().white()),
        cursor::MoveTo(i, 7),
        style::PrintStyledContent(format!("Score  : {}", game.score).bold().white()),
        cursor::MoveTo(i, 8),
        style::PrintStyledContent(format!("High   : {}", game.high_score).bold().white()),
        cursor::MoveTo(i, 9),
        style::PrintStyledContent(format!("Level  : {}", game.level + 1).bold().white()),
    )?;
    game.draw_message_at(
        w,
        Position::from_xy(game.board.width - 1, game.board.height),
        level2bonus(game.level).0,
    )?;

    let s = vec!['\u{1F642}'; game.lives as usize];
    let s1 = vec![' '; MAX_PACMAN_LIVES as usize - s.len()];
    let s2: String = s.into_iter().chain(s1).collect::<String>();
    game.draw_message_at(w, Position::from_xy(0, game.board.height), &s2)?;

    // scroll marquee
    let (cols, rows) = match terminal::size() {
        Ok((cols, rows)) => (cols, rows),
        Err(_) => (0, 0), // panic!
    };

    let marquee_x = 0; // start column
    let q: u16 = cols.saturating_sub(1); // Subtract 1 to avoid the "last cell" scroll trigger
    let i1: usize = game.mq_idx % MARQUEE.len();
    let t: usize = q as usize + game.mq_idx;
    let i2: usize = t % MARQUEE.len();

    crossterm::queue!(w, cursor::MoveTo(marquee_x, rows - 1))?;
    if i1 < i2 {
        crossterm::queue!(w, style::PrintStyledContent(MARQUEE[i1..i2].white()))?;
    } else {
        // marquee is assumed to be ascii (1 byte characters) 
        let part1 = &MARQUEE[i1..];
        let part2 = &MARQUEE[0..i2.min(MARQUEE.len())];
        crossterm::queue!(
            w,
            style::PrintStyledContent(format!("{}{}", part1, part2).white())
        )?;
    }
    Ok(())
}

fn draw_board<W: Write>(w: &mut W, game: &Game, bold: bool) -> io::Result<()> {
    for col in 0..game.board.width {
        for row in 0..game.board.height {
            let p = Position::from_xy(col, row);
            let s = match game.board[p] {
                Square::Wall => "#".blue(),
                Square::Dot => ".".white(),
                Square::Pill => "*".slow_blink(),
                _ => " ".white(),
            };
            let s = if bold { s.bold() } else { s };
            crossterm::queue!(
                w,
                cursor::MoveTo(col as u16, row as u16),
                style::PrintStyledContent(s),
            )?;
        }
    }

    // print fruit separately - because not rendered correctly otherwise (is wider than one cell)
    if game.fruit_duration > 0 {
        let fruit = level2bonus(game.level).0;
        let (col, row) = (game.board.fruit.col(), game.board.fruit.row());
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(col as u16, row as u16),
            style::Print(fruit),
        )?;
    }
    Ok(())
}

fn flash_board(game: &Game) -> io::Result<()> {
    for i in 0..10 {
        let mut w = io::BufWriter::new(stdout());
        draw_board(&mut w, game, i % 2 == 0)?;
        w.flush()?;
        thread::sleep(time::Duration::from_millis(300));
    }
    Ok(())
}

fn draw_player<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
    let ch = match game.player.last_input_direction {
        Left => ['}', ')', '>', '-', '>', ')'],
        Right => ['{', '(', '<', '-', '<', '('],
        Up => ['V', 'V', 'V', 'V', '|', '|'],
        Down => ['^', '^', '^', '^', '|', '|'],
    }[game.player.anim_frame];
    let (col, row) = (game.player.pos.col() as u16, game.player.pos.row() as u16);
    crossterm::queue!(
        w,
        cursor::MoveTo(col, row),
        style::PrintStyledContent(ch.bold().yellow()),
    )
}

fn draw_ghosts<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
    //"\u{1F631}".rapid_blink() // looks bad
    for (i, g) in game.ghosts.iter().enumerate() {
        let s = match (g.state, game.board[g.pos] != Square::House, i) {
            (GhostState::Dead, _, _) => "\u{1F440}", // Eyes
            (_, true, _) if (1..2000).contains(&g.edible_duration) => "\u{1F47D}", // Alien
            (_, true, _) if g.edible_duration > 0 => "\u{1F631}", // Scream
            (_, _, 0) => "\u{1F47A}",                // Goblin
            (_, _, 1) => "\u{1F479}",                // Ogre
            (_, _, 2) => "\u{1F47B}",                // Ghost
            (_, _, _) => "\u{1F383}",                // Jack-O-Lantern
        };
        let (col, row) = (g.pos.col() as u16, g.pos.row() as u16);
        crossterm::queue!(w, cursor::MoveTo(col, row), style::Print(s),)?;
    }
    Ok(())
}

//  The animated death and flashing screen happen syncronously. To be done
//  correctly, they should be pseudo-event driven like the rest of the program.
fn draw_dynamic(game: &Game) -> io::Result<()> {
    let mut w = io::BufWriter::new(stdout());
    draw_board(&mut w, game, false)?;
    draw_player(&mut w, game)?;
    draw_ghosts(&mut w, game)?;
    render_rhs(&mut w, game)?;
    w.flush()
}

fn game_loop(game: &mut Game) -> io::Result<GameState> {
    loop {
        let start = time::Instant::now();

        // adjust overall speed by level
        let base_speed = match game.level {
            0 => 140,
            1..=3 => 130,
            _ => 120,
        };
        // faster if power pill eaten
        let speed_boost = if game.ghosts.iter().any(|g| g.edible_duration > 0) {
            20
        } else {
            0
        };
        thread::sleep(time::Duration::from_millis(base_speed - speed_boost));

        if let Ok(true) = poll(time::Duration::from_millis(10)) {
            if let Ok(Event::Key(key_event)) = read() {
                // Filter out Release/Repeat events for Windows compatibility
                if key_event.kind == crossterm::event::KeyEventKind::Press {
                    game.player.last_input_direction = match key_event.code {
                        KeyCode::Char('q') => return Ok(GameState::UserQuit),
                        KeyCode::Char('v') => {
                            game.ghosts_are_edible(game.pill_duration); // cheat
                            game.player.last_input_direction
                        }
                        KeyCode::Left => Left,
                        KeyCode::Right => Right,
                        KeyCode::Up => Up,
                        KeyCode::Down => Down,
                        KeyCode::Char(' ') => {
                            game.pause()?;
                            game.player.last_input_direction
                        }
                        _ => game.player.last_input_direction,
                    }
                }
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
    sounds: [StaticSoundData; AUDIO_FILES.len()],
}

impl AM {
    fn play(&mut self, name: Sound) -> Result<StaticSoundHandle, std::io::Error> {
        self.manager
            .play(self.sounds[name as usize].clone())
            .map_err(io::Error::other)
    }
}

fn main_game() -> io::Result<()> {
    let mut game = Game::new();
    render_game_info()?;
    loop {
        draw_dynamic(&game)?;
        {
            let mut w = io::BufWriter::new(stdout());
            game.draw_message(&mut w, "READY!", false)?;
        }
        thread::sleep(time::Duration::from_millis(1200));

        match game_loop(&mut game)? {
            GameState::UserQuit => break,
            GameState::SheetComplete => {
                game.am.play(Sound::OpeningSong).map_err(io::Error::other)?;
                flash_board(&game)?;
                game.level += 1;
                // clear screen - next board may have different height
                crossterm::queue!(stdout(), terminal::Clear(terminal::ClearType::All),)?;
                game.repopulate_board();
                game.reset_ghosts();
                render_game_info()?;
                game.player = Player::new(&game.board);
                game.timecum = 0;
            }
            GameState::LifeLost => {
                let mut w = io::BufWriter::new(stdout());
                render_rhs(&mut w, &game)?;
                w.flush()?;
                game.am.play(Sound::Die).map_err(io::Error::other)?;
                animate_dead_player(&game)?;
                if game.lives == 0 {
                    break;
                }
                game.lives -= 1;
                thread::sleep(time::Duration::from_millis(100));
                game.reset_ghosts();
                game.player = Player::new(&game.board);
            }
        };
    }
    {
        let mut w = io::BufWriter::new(stdout());
        game.draw_message(&mut w, "GAME  OVER", true)
    }
}

fn main() -> io::Result<()> {
    // make sure crossterm doesn't leave the terminal in a raw state in case of panics
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = close_render();
        println!("panicked");
        original_hook(panic_info);
    }));

    init_render()?;
    loop {
        main_game()?;
        if !another_game()? {
            break;
        }
    }
    close_render()
}
