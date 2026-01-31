use rand::random;
use std::io::{self, Write, stdout};
use std::{thread, time};

mod audio;
mod board;
mod maze;
mod tui;
use audio::{AM, Sound};
use board::{Board, Direction, Direction::*, Position, Square};

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

const MAX_PACMAN_LIVES: u32 = 6;
fn pct(n: u8) -> bool {
    random::<u8>() % 100 < n
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

impl Game {
    fn new() -> Self {
        let level = 0u32;
        let board = Board::new(level);
        let player = Player::new(board.pacman_start);
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
        self.ghosts = self.board.ghost_start.map(Ghost::new);
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
                        tui::draw_message_at(&mut w, self, self.player.pos, &format!("{score}"))?;
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
                let bonus = self.bonus().value();
                self.score += bonus;
                self.fruit_duration = 0;

                {
                    let mut w = io::BufWriter::new(stdout());
                    tui::draw_message(&mut w, self, &format!("{bonus}"), false)?;
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

fn game_loop(game: &mut Game) -> io::Result<GameState> {
    let mut flash_frames_left: Option<u32> = None;
    let mut death_frames_left: Option<usize> = None;

    loop {
        let start = time::Instant::now();

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
        thread::sleep(time::Duration::from_millis(base_speed - speed_boost));

        let anim = flash_frames_left.is_some() || death_frames_left.is_some();
        match tui::poll_input()? {
            tui::InputEvent::Quit => return Ok(GameState::UserQuit),
            tui::InputEvent::Pause => tui::pause(game)?,
            _ if anim => (),
            tui::InputEvent::Cheat => game.ghosts_are_edible(game.pill_duration),
            tui::InputEvent::Direction(dir) => game.player.last_input_direction = dir,
            tui::InputEvent::None => {}
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
                game.am.play(Sound::Die).map_err(io::Error::other)?;
                death_frames_left = Some(12); // Start 12-frame death animation
            }

            if game.dots_left == 0 {
                game.am.play(Sound::OpeningSong).map_err(io::Error::other)?;
                flash_frames_left = Some(10);
            }

            // Fruit Logic
            if matches!(game.dots_left, 74 | 174) {
                game.fruit_duration = 1000 * (10 + random::<u128>() % 3);
                game.dots_left -= 1;
            }
        }

        //match flash_frames_left {
        //    Some(0) => return Ok(GameState::SheetComplete),
        //    Some(count) => {
        //        let mut w = io::BufWriter::new(stdout());
        //        tui::draw_board(&mut w, game, count % 2 == 0)?;
        //        tui::render_rhs(&mut w, game)?;
        //        w.flush()?;
        //        flash_frames_left = Some(count - 1)
        //    }
        //    None => {
        //        game.update((time::Instant::now() - start).as_millis())?;
        //        tui::draw_dynamic(game)?;

        //        if game.player.dead {
        //            return Ok(GameState::LifeLost);
        //        }

        //        if game.dots_left == 0 {
        //            game.am.play(Sound::OpeningSong).map_err(io::Error::other)?;
        //            flash_frames_left = Some(10);
        //        }

        //        // Fruit spawn logic
        //        if matches!(game.dots_left, 74 | 174) {
        //            game.fruit_duration = 1000 * (10 + random::<u128>() % 3);
        //            game.dots_left -= 1;
        //        }
        //    }
        //}
    }
}

fn main_game() -> io::Result<()> {
    let mut game = Game::new();
    tui::render_game_info()?;
    loop {
        tui::draw_dynamic(&game)?;
        {
            let mut w = io::BufWriter::new(stdout());
            tui::draw_message(&mut w, &game, "READY!", false)?;
        }
        thread::sleep(time::Duration::from_millis(1200));

        match game_loop(&mut game)? {
            GameState::UserQuit => break,
            GameState::SheetComplete => {
                game.level += 1;
                game.repopulate_board();
                tui::clear_screen()?; // next board may have different height
                tui::render_game_info()?;
                game.reset_ghosts();
                game.player = Player::new(game.board.pacman_start);
                game.timecum = 0;
            }
            GameState::LifeLost => {
                if game.lives == 0 {
                    break;
                }
                game.lives -= 1;
                thread::sleep(time::Duration::from_millis(100));
                game.reset_ghosts();
                game.player = Player::new(game.board.pacman_start);
            }
        };
    }
    let mut w = io::BufWriter::new(stdout());
    tui::draw_message(&mut w, &game, "GAME  OVER", true)
}

fn main() -> io::Result<()> {
    // make sure crossterm doesn't leave the terminal in a raw state in case of panics
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = tui::close_render();
        println!("panicked");
        original_hook(panic_info);
    }));

    tui::init_render()?;
    loop {
        main_game()?;
        if !tui::another_game()? {
            break;
        }
    }
    tui::close_render()
}
