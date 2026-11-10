use clap::Parser;
use std::io::{self, Write, stdout};
use std::{thread, time};

mod audio;
mod board;
mod highscore;
mod intermission;
mod maze;
mod tui;
use audio::{AM, Sound};
use board::{Board, Direction, Direction::*, Position, Square};

#[derive(Parser, Debug)]
#[command(author, version, about = "UniPac - Pac-Man for the terminal", long_about = None)]
struct Config {
    #[arg(
        short,
        long,
        help = "Start level: 1 shortened maze, 2 regular maze, 3-6 Ms Pac-Man mazes",
        default_value_t = 1,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    level: u32,

    #[arg(
        long,
        help = "Play intermission N (1) and exit",
        value_parser = clap::value_parser!(u8).range(1..=1)
    )]
    intermission: Option<u8>,
}

static MARQUEE: &str = "Title: Plato and Socrates Discuss Pac-Man. \
    Scene: A garden in Athens. Plato and Socrates sit beneath an olive tree. \
    Socrates: Tell me, Plato, have you played this curious game called Pac-Man? \
    Plato: I have observed it. A yellow creature wanders through a maze, pursued by four ghosts. \
    Socrates: And what does it seek? \
    Plato: Pellets, apparently. It seems to have a rather limited ambition. \
    Socrates: And yet it pursues this ambition relentlessly. Is that not true of most men? \
    Plato: Perhaps. But surely there is more to the game than eating pellets? \
    Socrates: There are also ghosts. \
    Plato: Yes, I noticed. They seem rather hostile. \
    Socrates: Would you say they are evil? \
    Plato: They pursue Pac-Man without mercy. What else could they be? \
    Socrates: And if Pac-Man pursues pellets without mercy, is he not equally guilty? \
    Plato: You are suggesting that the distinction between hero and villain is merely a matter of perspective? \
    Socrates: I suggest only that we should examine our assumptions. \
    Plato: Then let us consider the maze. Surely it is an imitation of a perfect maze, an imperfect shadow of its ideal Form. \
    Socrates: And where might one find this perfect maze? \
    Plato: In the realm of Forms, naturally. \
    Socrates: Does it contain ghosts? \
    Plato: I should think not. \
    Socrates: Then how can it be a perfect Pac-Man maze? \
    Plato: You are being deliberately difficult. \
    Socrates: I am merely asking questions. \
    Plato: Consider the Power Pellet! It transforms Pac-Man from prey into predator. Surely this demonstrates that power is the foundation of justice? \
    Socrates: Or perhaps that the ghosts were never so frightening as we imagined. Tell me, does Pac-Man become wiser when he eats one? \
    Plato: No, merely more dangerous. \
    Socrates: Then perhaps power and wisdom are not the same thing. \
    Plato: You make even a simple game sound complicated. \
    Socrates: Is that not what philosophy is for? \
    Plato: But surely there is an ideal strategy, a sequence of perfect moves that guarantees victory? \
    Socrates: If a man memorises every move but cannot explain why he makes them, does he understand the game? \
    Plato: He knows how to win. \
    Socrates: I asked whether he understands it. \
    Plato: Must we distinguish between knowing and understanding even in Pac-Man? \
    Socrates: Especially in Pac-Man. \
    Plato: Very well. Let us play, and discover the truth for ourselves. \
    Socrates: An excellent suggestion. But you must promise not to blame the ghosts when you lose. \
    Plato: And you must promise not to question me while I am playing. \
    Socrates: Then I shall ask only one question: why did you turn left? \
    Plato: Oh, be quiet, Socrates! \
    (They sit before the game. Plato loses his first life almost immediately.) \
    Socrates: I see we have discovered another Form: the ideal player, who exists only in theory. \
    Fin.   ";

const MAX_PACMAN_LIVES: u32 = 6;
fn pct(n: u8) -> bool {
    rand::random_range(0..100) < n
}

enum GameState {
    UserQuit,
    SheetComplete,
    LifeLost,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum GhostState {
    Home,    // waiting in the ghost house
    Leaving, // released: on its way to the gate
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
    exit_right: bool, // leave the house to the right: the mode changed while waiting inside
    bob_ms: u128,     // time since a ghost waiting in the house last moved
}

// Pinky, Blinky, Inky & Clyde: index into Game::ghosts and Board::ghost_start
const PINKY: usize = 0;
const BLINKY: usize = 1;
const INKY: usize = 2;
const CLYDE: usize = 3;
// Which waiting ghost leaves first, and whose dot counter is running
const LEAVING_ORDER: [usize; 3] = [PINKY, INKY, CLYDE];
// A ghost waiting in the house moves up or down one row this often
const BOB_MS: u128 = 300;

impl Ghost {
    const fn new(pos: Position, state: GhostState) -> Self {
        Ghost {
            pos,
            direction: match state {
                GhostState::Home => Up, // bobbing up and down
                _ => Left,
            },
            edible_duration: 0,
            state,
            exit_right: false,
            bob_ms: 0,
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
        [Right, Down, Left, Up]
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
                    Some((target.dist_sqr(p) as isize, d, p))
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
    fn new(pacman_start: Position) -> Player {
        Player {
            pos: pacman_start,
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
    // Rules for when ghosts leave the house (as in the arcade game):
    dot_counters: [u32; 4], // dots eaten while a ghost was first in line to leave
    global_counter: Option<u32>, // replaces the personal counters after a life is lost
    dot_timer: u128,        // time since pacman last ate a dot
    last_period: Period,    // to notice when the mode changes
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

#[derive(Copy, Clone, Debug, PartialEq)]
enum Fruit {
    Cherries,
    Strawberry,
    Peach,
    RedApple,
    Grapes,
    Galaxian,
    Bell,
    Key,
}

impl Fruit {
    pub fn value(&self) -> u32 {
        match self {
            Self::Cherries => 100,
            Self::Strawberry => 300,
            Self::Peach => 500,
            Self::RedApple => 700,
            Self::Grapes => 1000,
            Self::Galaxian => 2000,
            Self::Bell => 3000,
            Self::Key => 5000,
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        let level = 0u32;
        Game::new(level)
    }
}

impl Game {
    fn new(level: u32) -> Self {
        let board = Board::new(level);
        let player = Player::new(board.pacman_start);
        let mut game = Game {
            timecum: 0,
            mq_idx: 0,
            ghosts: [Ghost::new(Position::from_xy(0, 0), GhostState::Home); 4],
            dot_counters: [0; 4],
            global_counter: None,
            dot_timer: 0,
            last_period: Period::Scatter,
            pill_duration: 6000,
            level,
            board,
            dots_left: 0,
            high_score: highscore::load(),
            lives: 3,
            player,
            fruit_duration: 0,
            next_ghost_score: 0,
            score: 0,
            am: AM::default(),
        };
        game.reset_ghosts();
        game.repopulate_board();
        game
    }

    fn bonus(&self) -> Fruit {
        match self.level {
            0 => Fruit::Cherries,
            1 => Fruit::Strawberry,
            2 | 3 => Fruit::Peach,
            4 | 5 => Fruit::RedApple,
            6 | 7 => Fruit::Grapes,
            8 | 9 => Fruit::Galaxian,
            10 | 11 => Fruit::Bell,
            _ => Fruit::Key,
        }
    }

    fn reset_ghosts(&mut self) {
        self.ghosts = self
            .board
            .ghost_start
            .map(|p| Ghost::new(p, GhostState::Home));
        // Blinky starts outside the house
        self.ghosts[BLINKY].state = GhostState::Outside;
    }

    /// Start of a level (`life_lost` false) or of a new life.
    /// Call after the scatter/chase timer has been set.
    fn reset_house_rules(&mut self, life_lost: bool) {
        self.dot_timer = 0;
        self.last_period = self.period();
        if life_lost {
            // the personal counters keep their values but are not used again until Clyde
            // leaves; a global counter decides when the others leave
            self.global_counter = Some(0);
        } else {
            self.dot_counters = [0; 4];
            self.global_counter = None;
        }
    }

    /// Dots a ghost must wait for before leaving the house, while it is first in line.
    /// (The Pac-Man Dossier, "Home Sweet Home".)
    fn dot_limit(&self, ghost: usize) -> u32 {
        match (ghost, self.level) {
            (INKY, 0) => 30,
            (CLYDE, 0) => 60,
            (CLYDE, 1) => 50,
            _ => 0, // Pinky always, and everyone from the third level
        }
    }

    /// The ghost that will be the next to leave: the first in line inside the house
    fn next_in_house(&self) -> Option<usize> {
        LEAVING_ORDER
            .into_iter()
            .find(|&i| self.ghosts[i].state == GhostState::Home)
    }

    fn release(&mut self, ghost: usize) {
        self.ghosts[ghost].state = GhostState::Leaving;
    }

    /// Pacman ate a dot or an energizer
    fn dot_eaten(&mut self) {
        self.dot_timer = 0;
        if let Some(n) = self.global_counter {
            let n = n + 1;
            self.global_counter = Some(n);
            let ghost = match n {
                7 => PINKY,
                17 => INKY,
                32 => CLYDE,
                _ => return,
            };
            if self.ghosts[ghost].state == GhostState::Home {
                self.release(ghost);
                if ghost == CLYDE {
                    self.global_counter = None; // the personal counters are used again
                }
            }
            // (if Clyde is not inside at 32 the counter just goes on - as in the arcade game)
        } else if let Some(ghost) = self.next_in_house() {
            self.dot_counters[ghost] += 1;
            self.release_ghosts_at_limit();
        }
    }

    fn release_ghosts_at_limit(&mut self) {
        if self.global_counter.is_some() {
            return;
        }
        while let Some(ghost) = self.next_in_house() {
            if self.dot_counters[ghost] < self.dot_limit(ghost) {
                break;
            }
            self.release(ghost);
        }
    }

    /// A pacman that avoids dots can't keep the ghosts in the house forever
    fn tick_release_timer(&mut self, telaps: u128) {
        self.dot_timer += telaps;
        let limit = if self.level >= 4 { 3000 } else { 4000 };
        if self.dot_timer >= limit {
            self.dot_timer = 0;
            if let Some(ghost) = self.next_in_house() {
                self.release(ghost);
            }
        }
    }

    /// Ghosts inside the house leave to the right, not the left, if the mode has changed
    fn mode_changed(&mut self) {
        for g in self.ghosts.iter_mut() {
            if matches!(g.state, GhostState::Home | GhostState::Leaving) {
                g.exit_right = true;
            }
        }
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
        self.mode_changed();
        for g in self.ghosts.iter_mut() {
            if matches!(g.state, GhostState::Outside | GhostState::Gateway) {
                // a pill eaten while ghosts are still frightened restarts the
                // timer (as in the original) rather than adding to it
                g.edible_duration = duration;
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
                    self.am.play(Sound::EatGhost);

                    // flash message
                    let mut w = io::BufWriter::new(stdout());
                    tui::draw_message_at(&mut w, self, self.player.pos, &format!("{score}"))?;
                    thread::sleep(time::Duration::from_millis(150));
                }
            }
        }
        Ok(())
    }

    /// Target tiles for Pinky, Blinky, Inky & Clyde (in ghost index order):
    /// (scatter targets, chase targets).
    fn ghost_targets(&self) -> ([Position; 4], [Position; 4]) {
        let (width, height) = (self.board.width, self.board.height);
        let scatter_target = [
            Position::from_xy(2, 0),              // Pinky: top left
            Position::from_xy(width - 3, 0),      // Blinky: top right
            Position::from_xy(width - 1, height), // Inky: bottom right
            Position::from_xy(0, height),         // Clyde: bottom left
        ];

        // Targets may lie outside the maze in the original - clamp to the board
        let tile = |col: i32, row: i32| {
            Position::from_xy(
                col.clamp(0, width as i32 - 1) as usize,
                row.clamp(0, height as i32) as usize,
            )
        };
        // the tile `n` ahead of pacman. When pacman moves up the original also
        // shifts the tile `n` to the left (an overflow bug in the arcade code)
        let (col, row) = (self.player.pos.col() as i32, self.player.pos.row() as i32);
        let ahead = |n: i32| match self.player.moving {
            Left => (col - n, row),
            Right => (col + n, row),
            Up => (col - n, row - n),
            Down => (col, row + n),
        };

        let mut chase_target = [self.player.pos; 4];
        // Blinky - target pacman
        // Pinky - target 4 tiles ahead of pacman
        let (c, r) = ahead(4);
        chase_target[0] = tile(c, r);
        // Inky - twice the vector from Blinky to the tile 2 ahead of pacman
        let (c, r) = ahead(2);
        let blinky = self.ghosts[1].pos;
        chase_target[2] = tile(2 * c - blinky.col() as i32, 2 * r - blinky.row() as i32);
        // Clyde - retreat to corner if close to pacman
        if self.player.pos.dist_sqr(self.ghosts[3].pos) < 64 {
            chase_target[3] = scatter_target[3]
        }
        (scatter_target, chase_target)
    }

    fn update_ghosts(&mut self, telaps: u128) {
        let (scatter_target, chase_target) = self.ghost_targets();
        let current_period = self.period();
        if current_period != self.last_period {
            self.last_period = current_period;
            self.mode_changed();
        }
        // Blinky has no dot counter - he leaves at once, e.g. after being eaten
        if self.ghosts[BLINKY].state == GhostState::Home {
            self.release(BLINKY);
        }
        self.release_ghosts_at_limit();
        self.tick_release_timer(telaps);
        for (gidx, g) in self.ghosts.iter_mut().enumerate() {
            g.edible_duration = g.edible_duration.saturating_sub(telaps);
            (g.direction, g.pos) = match g.state {
                GhostState::Home => {
                    // bob up and down while waiting
                    g.bob_ms += telaps;
                    if g.bob_ms < BOB_MS {
                        (g.direction, g.pos)
                    } else {
                        g.bob_ms -= BOB_MS;
                        let inside = |p| matches!(self.board[p], Square::House | Square::Empty);
                        let dir = if inside(g.pos.go(g.direction)) {
                            g.direction
                        } else {
                            g.direction.opposite()
                        };
                        (dir, g.pos.go(dir))
                    }
                }
                GhostState::Leaving => {
                    // sideways to the gate, then up through it
                    let gate_col = self.board.gate1.col();
                    if g.pos.col() != gate_col {
                        let dir = if g.pos.col() < gate_col { Right } else { Left };
                        (dir, g.pos.go(dir))
                    } else {
                        let pos = g.pos.go(Up);
                        if self.board[pos] == Square::Gate {
                            g.state = GhostState::Gateway;
                        }
                        (Up, pos)
                    }
                }
                GhostState::Gateway => {
                    g.state = GhostState::Outside;
                    let dir = if g.exit_right { Right } else { Left };
                    (dir, g.pos.go(Up))
                }
                GhostState::Dead => {
                    if g.pos == self.board.gate1 || g.pos == self.board.gate2 {
                        // revived: waits (bobbing) until released like the others
                        g.state = GhostState::Home;
                        g.exit_right = false;
                        g.bob_ms = 0;
                        (Down, g.pos.go(Down))
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
        self.timecum += dur;
        self.fruit_duration = self.fruit_duration.saturating_sub(dur);
        Ok(())
    }

    fn move_player(&mut self, pos: Position) -> io::Result<bool> {
        // move may not be valid - return true if valid
        match self.board[pos] {
            Square::Dot => {
                self.score += 10;
                self.dots_left -= 1;
                self.board[pos] = Square::Empty;
                self.dot_eaten();
            }
            Square::Pill => {
                self.am.play(Sound::EatPill);
                self.board[pos] = Square::Empty;
                self.dot_eaten();
                self.ghosts_are_edible(self.pill_duration);
                self.score += 50;
                self.next_ghost_score = 200;
            }
            Square::Fruit if self.fruit_duration > 0 => {
                self.am.play(Sound::EatPill);
                let bonus = self.bonus().value();
                self.score += bonus;
                self.fruit_duration = 0;

                // flash message
                let mut w = io::BufWriter::new(stdout());
                tui::draw_message(&mut w, self, &format!("{bonus}"), false)?;
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
            self.am.play(Sound::ExtraLives);
        }

        if self.score > self.high_score {
            self.high_score = self.score;
        }
        Ok(())
    } // update_player

    // one full game
    fn run(&mut self) -> io::Result<()> {
        tui::render_game_info()?;
        loop {
            tui::draw_dynamic(self)?;
            // flash message
            let mut w = io::BufWriter::new(stdout());
            tui::draw_message(&mut w, self, "READY!", false)?;
            thread::sleep(time::Duration::from_millis(1200));

            match game_loop(self)? {
                GameState::UserQuit => break,
                GameState::SheetComplete => {
                    if let Some(act) = intermission::act_after(self.level)
                        && !tui::intermission(self, act)?
                    {
                        break; // quit during the intermission
                    }
                    self.level += 1;
                    self.repopulate_board();
                    tui::clear_screen()?; // next board may have different height
                    tui::render_game_info()?;
                    self.reset_ghosts();
                    self.player = Player::new(self.board.pacman_start);
                    self.timecum = 0;
                    self.reset_house_rules(false);
                }
                GameState::LifeLost => {
                    if self.lives == 0 {
                        break;
                    }
                    self.lives -= 1;
                    thread::sleep(time::Duration::from_millis(100));
                    self.reset_ghosts();
                    self.player = Player::new(self.board.pacman_start);
                    self.reset_house_rules(true);
                }
            };
        }
        highscore::save(self.high_score);
        tui::draw_game_over(self)
    }
} // impl Game

fn game_loop(game: &mut Game) -> io::Result<GameState> {
    let mut flash_frames_left: Option<u32> = None;
    let mut death_frames_left: Option<usize> = None;

    loop {
        let mut start = time::Instant::now();

        // adjust overall speed by level
        let base_speed = if flash_frames_left.is_some() {
            300
        } else if death_frames_left.is_some() {
            150
        } else {
            match game.level {
                0 => 140,
                1..=3 => 130,
                _ => 120,
            }
        };
        // faster if power pill eaten
        let speed_boost = if flash_frames_left.is_none()
            && death_frames_left.is_none()
            && game.ghosts.iter().any(|g| g.edible_duration > 0)
        {
            20
        } else {
            0
        };
        let target_frame_time = time::Duration::from_millis(base_speed - speed_boost);
        if start.elapsed() < target_frame_time {
            thread::sleep(target_frame_time - start.elapsed());
        }

        // Drain all queued input: reading one event per frame falls behind
        // key repeat (~30/s vs ~7 frames/s), so steering lags more and more.
        // The first poll waits up to 10 ms, as before; the rest don't wait.
        let mut timeout = time::Duration::from_millis(10);
        let mut resized = false;
        while let Some(event) = tui::poll_input(timeout)? {
            timeout = time::Duration::ZERO;
            match event {
                tui::InputEvent::Quit => return Ok(GameState::UserQuit),
                tui::InputEvent::Pause => {
                    if tui::pause(game)? {
                        return Ok(GameState::UserQuit);
                    }
                    // time spent paused must not count as game time (it would
                    // drain the power pill and fruit timers): make this frame
                    // look like a normal one
                    start = time::Instant::now()
                        .checked_sub(target_frame_time)
                        .unwrap_or_else(time::Instant::now);
                }
                tui::InputEvent::Resize => resized = true,
                // several queued directions: the last one wins
                tui::InputEvent::Direction(dir) => game.player.last_input_direction = dir,
                _ => {}
            }
        }
        if resized {
            tui::clear_screen()?;
            tui::render_game_info()?; // Re-draw titles and scores
        }

        game.mq_idx = (game.mq_idx + 1) % MARQUEE.len(); // scroll marquee

        let mut w = io::BufWriter::new(stdout());
        if let Some(count) = flash_frames_left {
            // --- VICTORY FLASH ---
            if count == 0 {
                return Ok(GameState::SheetComplete);
            }
            tui::draw_board(&mut w, game, count % 2 == 0)?;
            tui::render_rhs(&mut w, game)?;
            w.flush()?;
            flash_frames_left = Some(count - 1);
        } else if let Some(count) = death_frames_left {
            // --- DEATH ANIMATION ---
            if count == 0 {
                return Ok(GameState::LifeLost);
            }
            tui::draw_board(&mut w, game, false)?;
            tui::draw_death_frame(&mut w, game, 12 - count)?;
            tui::render_rhs(&mut w, game)?;
            w.flush()?;
            death_frames_left = Some(count - 1);
        } else {
            // --- NORMAL GAMEPLAY ---
            game.update((time::Instant::now() - start).as_millis())?;
            tui::draw_dynamic(game)?;

            if game.player.dead {
                game.am.play(Sound::Die);
                death_frames_left = Some(12); // Start 12-frame death animation
            }

            if game.dots_left == 0 {
                game.am.play(Sound::OpeningSong);
                flash_frames_left = Some(10);
            }

            // Fruit Logic
            if matches!(game.dots_left, 74 | 174) {
                game.fruit_duration = 1000 * (10 + rand::random_range(0..3));
                game.dots_left -= 1;
            }
        }
    }
}

fn main() -> io::Result<()> {
    let args = Config::parse();

    // make sure crossterm doesn't leave the terminal in a raw state in case of panics
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = tui::close_render();
        println!("panicked");
        original_hook(panic_info);
    }));

    tui::init_render()?;
    if let Some(act) = args.intermission.and_then(intermission::Act::from_number) {
        let result = tui::intermission(&mut Game::new(args.level - 1), act);
        tui::close_render()?;
        return result.map(|_| ());
    }
    let mut mq_idx = 0;
    loop {
        let mut game = Game::new(args.level - 1); // levels are 1-based on the command line and in the panel
        game.mq_idx = mq_idx; // the marquee carries on where the last game left it
        game.run()?;
        if !tui::another_game(&mut game)? {
            break;
        }
        mq_idx = game.mq_idx;
    }
    tui::close_render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marquee_is_ascii() {
        // draw_marquee slices it by bytes, which panics on a multi-byte character
        // (curly quotes and dashes from ChatGPT, say)
        assert!(MARQUEE.is_ascii());
        assert!(MARQUEE.ends_with(' ')); // a gap where it wraps around
    }

    fn xy(p: Position) -> (usize, usize) {
        (p.col(), p.row())
    }

    #[test]
    fn ghost_targets() {
        let mut g = Game::default();
        let (w, h) = (g.board.width, g.board.height);
        g.player.pos = Position::from_xy(10, 10);
        g.ghosts[1].pos = Position::from_xy(8, 12); // Blinky
        g.ghosts[3].pos = Position::from_xy(10, 25); // Clyde, far away

        let (scatter, _) = g.ghost_targets();
        let corners = [(2, 0), (w - 3, 0), (w - 1, h), (0, h)];
        assert_eq!(scatter.map(xy), corners);

        g.player.moving = Right;
        let (_, chase) = g.ghost_targets();
        assert_eq!(xy(chase[1]), (10, 10)); // Blinky: pacman
        assert_eq!(xy(chase[0]), (14, 10)); // Pinky: 4 ahead
        assert_eq!(xy(chase[2]), (16, 8)); // Inky: 2*(12,10) - (8,12)
        assert_eq!(xy(chase[3]), (10, 10)); // Clyde: pacman when far away

        g.player.moving = Up; // the arcade overflow bug: also shifted left
        let (_, chase) = g.ghost_targets();
        assert_eq!(xy(chase[0]), (6, 6));
        assert_eq!(xy(chase[2]), (2 * 8 - 8, 2 * 8 - 12));

        g.ghosts[3].pos = Position::from_xy(12, 12); // Clyde within 8 tiles
        let (_, chase) = g.ghost_targets();
        assert_eq!(xy(chase[3]), (0, h)); // retreats to its own corner

        // bottom corners follow the board height, not a fixed row
        g.board = Board::new(1);
        let (scatter, _) = g.ghost_targets();
        assert_eq!(xy(scatter[2]), (w - 1, g.board.height));
        assert_eq!(xy(scatter[3]), (0, g.board.height));
    }

    fn eat_dots(g: &mut Game, n: u32) {
        for _ in 0..n {
            g.dot_eaten();
        }
    }

    fn states(g: &Game) -> [GhostState; 4] {
        g.ghosts.map(|g| g.state)
    }

    #[test]
    fn level_1_ghosts_leave_after_0_30_and_then_60_more_dots() {
        use GhostState::*;
        let mut g = Game::new(0);
        assert_eq!(states(&g), [Home, Outside, Home, Home]); // Blinky starts outside
        g.release_ghosts_at_limit();
        assert_eq!(states(&g), [Leaving, Outside, Home, Home]); // Pinky: no wait

        eat_dots(&mut g, 29);
        assert_eq!(g.ghosts[INKY].state, Home);
        eat_dots(&mut g, 1);
        assert_eq!(states(&g), [Leaving, Outside, Leaving, Home]); // Inky: 30 dots

        eat_dots(&mut g, 59);
        assert_eq!(g.ghosts[CLYDE].state, Home);
        eat_dots(&mut g, 1);
        assert_eq!(g.ghosts[CLYDE].state, Leaving); // Clyde: 60 dots after Inky left
    }

    #[test]
    fn level_2_and_3_limits() {
        use GhostState::*;
        let mut g = Game::new(1);
        g.release_ghosts_at_limit();
        assert_eq!(states(&g), [Leaving, Outside, Leaving, Home]); // Inky no longer waits
        eat_dots(&mut g, 49);
        assert_eq!(g.ghosts[CLYDE].state, Home);
        eat_dots(&mut g, 1);
        assert_eq!(g.ghosts[CLYDE].state, Leaving); // Clyde: 50

        let mut g = Game::new(2);
        g.release_ghosts_at_limit();
        assert_eq!(states(&g), [Leaving, Outside, Leaving, Leaving]); // nobody waits
    }

    #[test]
    fn no_dots_for_a_while_releases_the_next_ghost() {
        use GhostState::*;
        let mut g = Game::new(0);
        g.release_ghosts_at_limit(); // Pinky out
        g.tick_release_timer(3999);
        assert_eq!(g.ghosts[INKY].state, Home);
        g.tick_release_timer(1);
        assert_eq!(states(&g), [Leaving, Outside, Leaving, Home]); // after 4 s
        g.tick_release_timer(4000);
        assert_eq!(g.ghosts[CLYDE].state, Leaving);

        // every dot restarts the timer
        let mut g = Game::new(0);
        g.release_ghosts_at_limit();
        g.tick_release_timer(3000);
        g.dot_eaten();
        g.tick_release_timer(3000);
        assert_eq!(g.ghosts[INKY].state, Home);

        // from level 5 the wait is 3 s
        let mut g = Game::new(4);
        g.reset_ghosts();
        g.dot_counters = [0, 0, 0, 0];
        g.ghosts[PINKY].state = Leaving;
        g.tick_release_timer(2999);
        assert_eq!(g.ghosts[INKY].state, Home);
        g.tick_release_timer(1);
        assert_eq!(g.ghosts[INKY].state, Leaving);
    }

    #[test]
    fn after_a_lost_life_a_global_counter_decides() {
        use GhostState::*;
        let mut g = Game::new(0);
        g.dot_counters[INKY] = 5;
        g.reset_ghosts();
        g.reset_house_rules(true);
        g.release_ghosts_at_limit(); // the personal limits are not used now
        assert_eq!(states(&g), [Home, Outside, Home, Home]);

        eat_dots(&mut g, 6);
        assert_eq!(g.ghosts[PINKY].state, Home);
        eat_dots(&mut g, 1);
        assert_eq!(g.ghosts[PINKY].state, Leaving); // 7
        eat_dots(&mut g, 9);
        assert_eq!(g.ghosts[INKY].state, Home);
        eat_dots(&mut g, 1);
        assert_eq!(g.ghosts[INKY].state, Leaving); // 17
        eat_dots(&mut g, 14);
        assert_eq!(g.ghosts[CLYDE].state, Home);
        eat_dots(&mut g, 1);
        assert_eq!(g.ghosts[CLYDE].state, Leaving); // 32
        assert_eq!(g.global_counter, None); // the personal counters are in charge again
        assert_eq!(g.dot_counters[INKY], 5); // ... and were not reset by the lost life

        // Clyde isn't inside at 32: the global counter just goes on
        let mut g = Game::new(0);
        g.reset_house_rules(true);
        g.ghosts[CLYDE].state = Outside;
        eat_dots(&mut g, 32);
        assert_eq!(g.global_counter, Some(32));

        // a new level starts afresh
        g.reset_house_rules(false);
        assert_eq!((g.global_counter, g.dot_counters), (None, [0; 4]));
    }

    #[test]
    fn leaving_the_house() {
        let mut g = Game::new(0);
        let gate = g.board.front_of_gate1;
        // Pinky leaves at once, and to the left as the mode has not changed
        for _ in 0..6 {
            g.update_ghosts(150);
            if g.ghosts[PINKY].state == GhostState::Outside {
                break;
            }
        }
        let pinky = g.ghosts[PINKY];
        assert!(pinky.state == GhostState::Outside && pinky.pos == gate && pinky.direction == Left);

        // a ghost still inside when the mode changes leaves to the right instead
        let mut g = Game::new(0);
        g.timecum = 7000; // scatter -> chase
        g.update_ghosts(150);
        assert!(g.ghosts[INKY].exit_right && g.ghosts[CLYDE].exit_right);
        let mut g = Game::new(0);
        g.ghosts_are_edible(6000); // so does a pill
        assert!(g.ghosts[INKY].exit_right);

        // a ghost that was eaten waits inside again, and leaves as soon as it is its turn
        let mut g = Game::new(0);
        g.ghosts[BLINKY].state = GhostState::Home;
        g.ghosts[BLINKY].pos = g.board.gate1.go(Down);
        g.update_ghosts(150);
        assert_ne!(g.ghosts[BLINKY].state, GhostState::Home);
    }

    #[test]
    fn waiting_ghosts_bob_up_and_down() {
        let mut g = Game::new(0);
        let (col, row) = (g.ghosts[INKY].pos.col(), g.ghosts[INKY].pos.row());
        let mut rows = std::collections::BTreeSet::new();
        for _ in 0..20 {
            // 3 s: Inky is still waiting
            g.update_ghosts(150);
            assert_eq!(g.ghosts[INKY].state, GhostState::Home);
            assert_eq!(g.ghosts[INKY].pos.col(), col);
            rows.insert(g.ghosts[INKY].pos.row());
        }
        assert_eq!(rows, [row - 1, row, row + 1].into());
    }
}
