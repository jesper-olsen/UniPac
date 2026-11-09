use crate::{
    Fruit, Game, GhostState, MARQUEE, MAX_PACMAN_LIVES, Period, Position,
    board::{Direction, Square},
    maze::WIDTH,
};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, poll, read},
    style::{self, Stylize},
    terminal,
};
use std::io::{self, Write, stdout};
use std::time::Duration;

/// Terminal cells per maze tile. Cells are roughly twice as tall as wide, so
/// 2x1 cells per tile makes tiles close to square, and it is exactly the width
/// of the (East Asian Wide) emoji used for ghosts, fruit and lives.
const CELL_W: u16 = 2;

/// Screen column of the left cell of maze column `col`.
fn sx(col: usize) -> u16 {
    col as u16 * CELL_W
}

fn get_fruit_symbol(fruit: Fruit) -> &'static str {
    match fruit {
        Fruit::Cherries => "\u{1F352}",
        Fruit::Strawberry => "\u{1F353}",
        Fruit::Peach => "\u{1F351}",
        Fruit::RedApple => "\u{1F34E}",
        Fruit::Grapes => "\u{1F347}",
        Fruit::Galaxian => "\u{1F680}", // rocket ship
        Fruit::Bell => "\u{1F514}",
        Fruit::Key => "\u{1F511}",
    }
}

pub fn init_render() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    crossterm::execute!(
        stdout(),
        terminal::EnterAlternateScreen,
        terminal::Clear(terminal::ClearType::All),
        cursor::Hide,
        cursor::MoveTo(0, 0)
    )
}

pub fn close_render() -> io::Result<()> {
    crossterm::queue!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        terminal::LeaveAlternateScreen,
        cursor::Show,
        cursor::MoveTo(0, 0)
    )?;
    terminal::disable_raw_mode()
}

fn term_cols() -> u16 {
    terminal::size().map(|(c, _)| c).unwrap_or(80)
}

/// Start column for `s` centred in the right-hand panel of a terminal `cols` wide.
fn centered_in(cols: u16, s: &str) -> u16 {
    let leftedge: u16 = sx(WIDTH) + 2;
    let n: u16 = s.len() as u16;
    let offset = cols.saturating_sub(leftedge).saturating_sub(n) / 2;
    offset + leftedge
}

/// Column of the ':' in the info block ("Score  : 70").
fn colon_in(cols: u16) -> u16 {
    centered_in(cols, WIDEST_INFO) + 7
}

/// Start column for `s` centred over the ':' column, so that titles line up with the
/// info block whatever the terminal width. Stays clear of the maze.
fn over_colon_in(cols: u16, s: &str) -> u16 {
    let n: u16 = s.len() as u16;
    colon_in(cols).saturating_sub(n / 2).max(sx(WIDTH) + 1)
}

fn over_colon(s: &str) -> u16 {
    over_colon_in(term_cols(), s)
}

pub fn draw_message<W: Write>(w: &mut W, game: &Game, s: &str, blink: bool) -> io::Result<()> {
    let col = ((sx(game.board.width) as usize).saturating_sub(s.len()) / 2) as u16;
    let styled = if blink {
        s.bold().slow_blink()
    } else {
        s.bold()
    };
    crossterm::queue!(
        w,
        cursor::MoveTo(col, game.board.fruit.row() as u16),
        style::PrintStyledContent(styled)
    )?;
    w.flush()
}

pub fn draw_message_at<W: Write>(w: &mut W, game: &Game, pos: Position, s: &str) -> io::Result<()> {
    let (col, row) = (
        std::cmp::min(sx(pos.col()), sx(game.board.width) - 4),
        pos.row() as u16,
    );
    crossterm::queue!(
        w,
        cursor::MoveTo(col, row),
        style::PrintStyledContent(s.bold())
    )?;
    w.flush()
}

pub fn draw_dynamic(game: &Game) -> io::Result<()> {
    let mut w = io::BufWriter::new(stdout());
    draw_board(&mut w, game, false)?;
    draw_player(&mut w, game)?;
    draw_ghosts(&mut w, game)?;
    render_rhs(&mut w, game)?;
    w.flush()
}

/// The two terminal cells for a wall tile. The left cell is the box-drawing
/// glyph; the right cell continues the line east ('═') if the tile connects
/// to its right neighbour, so horizontal walls stay unbroken.
fn get_wall_char(board: &crate::board::Board, pos: Position) -> &'static str {
    let col = pos.col();
    let row = pos.row();

    let up = row > 0
        && matches!(
            board[Position::from_xy(col, row - 1)],
            Square::Wall | Square::Gate
        );
    let down = row < board.height - 1
        && matches!(
            board[Position::from_xy(col, row + 1)],
            Square::Wall | Square::Gate
        );
    let left = col > 0
        && matches!(
            board[Position::from_xy(col - 1, row)],
            Square::Wall | Square::Gate
        );
    let right = col < board.width - 1
        && matches!(
            board[Position::from_xy(col + 1, row)],
            Square::Wall | Square::Gate
        );

    match (up, down, left, right) {
        (true, true, true, true) => "╬═",
        (true, true, true, false) => "╣ ",
        (true, true, false, true) => "╠═",
        (true, false, true, true) => "╩═",
        (false, true, true, true) => "╦═",
        (true, true, false, false) => "║ ",
        (false, false, true, true) => "══",
        (true, false, true, false) => "╝ ",
        (true, false, false, true) => "╚═",
        (false, true, true, false) => "╗ ",
        (false, true, false, true) => "╔═",
        (true, _, _, _) => "║ ",
        (false, true, _, _) => "║ ",
        (_, _, true, _) => "═ ",
        (_, _, _, true) => "══",
        (false, false, false, false) => "██",
    }
}

pub fn draw_board<W: Write>(w: &mut W, game: &Game, bold: bool) -> io::Result<()> {
    for col in 0..game.board.width {
        for row in 0..game.board.height {
            let p = Position::from_xy(col, row);
            let s = match game.board[p] {
                //Square::Wall => "#".blue(),
                Square::Wall => get_wall_char(&game.board, p).blue(),
                //Square::Wall => "\u{2588}".blue(), // Full Block
                Square::Dot => ". ".white(),
                //Square::Pill => "*".slow_blink(),
                Square::Pill => "● ".slow_blink().white(), // Using a rounder dot for pills
                Square::Gate => "──".white(),
                // emoji are 2 cells wide - exactly one tile
                Square::Fruit if game.fruit_duration > 0 => get_fruit_symbol(game.bonus()).white(),
                _ => "  ".white(),
            };
            let s = if bold { s.bold() } else { s };
            crossterm::queue!(
                w,
                cursor::MoveTo(sx(col), row as u16),
                style::PrintStyledContent(s),
            )?;
        }
    }

    Ok(())
}

pub fn draw_player<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
    let ch = match game.player.last_input_direction {
        Direction::Left => ['}', ')', '>', '-', '>', ')'],
        Direction::Right => ['{', '(', '<', '-', '<', '('],
        Direction::Up => ['V', 'V', 'V', 'V', '|', '|'],
        Direction::Down => ['^', '^', '^', '^', '|', '|'],
    }[game.player.anim_frame];
    let (col, row) = (sx(game.player.pos.col()), game.player.pos.row() as u16);
    crossterm::queue!(
        w,
        cursor::MoveTo(col, row),
        style::PrintStyledContent(ch.bold().yellow())
    )
}

/// q, Esc or Ctrl-C. In raw mode Ctrl-C arrives as an ordinary key event
/// rather than as a signal, so it has to be handled explicitly.
fn is_quit(key: &event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(event::KeyModifiers::CONTROL))
}

pub enum InputEvent {
    Direction(Direction),
    Quit,
    Pause,
    Cheat,
    Resize,
    None,
}

/// Next pending input event, waiting up to `timeout` for one to arrive.
/// `None`: nothing pending. `Some(InputEvent::None)`: an event we ignore.
pub fn poll_input(timeout: Duration) -> io::Result<Option<InputEvent>> {
    if !poll(timeout)? {
        return Ok(None);
    }
    Ok(Some(match read()? {
        Event::Key(key_event) if key_event.kind == event::KeyEventKind::Press => {
            match key_event.code {
                _ if is_quit(&key_event) => InputEvent::Quit,
                KeyCode::Char('v') => InputEvent::Cheat,
                KeyCode::Char(' ') => InputEvent::Pause,
                KeyCode::Left => InputEvent::Direction(Direction::Left),
                KeyCode::Right => InputEvent::Direction(Direction::Right),
                KeyCode::Up => InputEvent::Direction(Direction::Up),
                KeyCode::Down => InputEvent::Direction(Direction::Down),
                _ => InputEvent::None,
            }
        }
        Event::Resize(_, _) => InputEvent::Resize,
        _ => InputEvent::None,
    }))
}

/// Blocks until unpaused. Returns true if the player asked to quit instead.
pub fn pause(game: &Game) -> io::Result<bool> {
    let mut w = io::BufWriter::new(stdout());
    draw_message(&mut w, game, "PAUSED", false)?;
    loop {
        match read() {
            // Filter out Release/Repeat events for Windows compatibility
            Ok(Event::Key(key_event))
                if key_event.kind == crossterm::event::KeyEventKind::Press =>
            {
                if is_quit(&key_event) {
                    return Ok(true);
                }
                if key_event.code == KeyCode::Char(' ') {
                    return Ok(false);
                }
            }
            Ok(Event::Resize(_, _)) => {
                redraw(game)?;
                draw_message(&mut w, game, "PAUSED", false)?;
            }
            _ => (),
        }
    }
}

pub fn another_game(game: &mut Game) -> io::Result<bool> {
    let s1 = "Another game, squire?";
    let s2 = "Y/N";

    loop {
        let mut w = io::BufWriter::new(stdout());
        draw_marquee(&mut w, game)?;
        crossterm::queue!(
            w,
            cursor::MoveTo(over_colon(s1), PROMPT_ROW),
            style::PrintStyledContent(s1.red()),
            cursor::MoveTo(over_colon(s2), PROMPT_ROW + 2),
            style::PrintStyledContent(s2.red()),
        )?;
        w.flush()?;
        game.mq_idx = (game.mq_idx + 1) % MARQUEE.len();

        // same pace as the game, which scrolls the marquee once per 150 ms frame
        if poll(Duration::from_millis(150))? {
            match read() {
                Ok(Event::Key(key_event))
                    if key_event.kind == crossterm::event::KeyEventKind::Press =>
                {
                    match key_event.code {
                        KeyCode::Char('y' | 'Y') => return Ok(true),
                        KeyCode::Char('n' | 'N') => return Ok(false),
                        _ if is_quit(&key_event) => return Ok(false),
                        _ => (),
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    redraw(game)?;
                    draw_game_over(game)?;
                }
                _ => (),
            }
        }
    }
}

const WIDEST_INFO: &str = "Maze   : Light Blue"; // the widest line of the info block
const TITLE: [&str; 3] = ["UniPac", "Terminal Pac-Man", "Rusty Edition 2026"];
const CONTROLS: [&str; 3] = ["Move   : arrows", "Pause  : space", "Quit   : q / Esc"];
// rows of the right-hand panel: info block ends on row 11
const PROMPT_ROW: u16 = 14; // 'Another game?' and 'Y/N' on this row and 2 below
const CONTROLS_ROW: u16 = 19;

/// Left edge of the text block in the right-hand panel: centred on its widest line,
/// so that no line runs past the edge of an 80 column terminal.
fn panel_x() -> u16 {
    centered_in(term_cols(), WIDEST_INFO)
}

pub fn render_game_info() -> io::Result<()> {
    // the panel right of the maze is 22 columns wide in an 80 column terminal
    let [s1, s2, s3] = TITLE;

    crossterm::queue!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(over_colon(s1), 1),
        style::PrintStyledContent(s1.cyan()),
        cursor::MoveTo(over_colon(s2), 2),
        style::PrintStyledContent(s2.cyan()),
        cursor::MoveTo(over_colon(s3), 3),
        style::PrintStyledContent(s3.yellow()),
    )?;
    // below the 'Another game?' prompt
    for (n, s) in CONTROLS.iter().enumerate() {
        crossterm::queue!(
            stdout(),
            cursor::MoveTo(panel_x(), CONTROLS_ROW + n as u16),
            style::PrintStyledContent(s.grey()),
        )?;
    }
    Ok(())
}

pub fn draw_death_frame<W: io::Write>(w: &mut W, game: &Game, frame_idx: usize) -> io::Result<()> {
    let animation = "|Vv_.+*X*+. ";
    let ch = animation.chars().nth(frame_idx).unwrap_or(' ');
    crossterm::queue!(
        w,
        cursor::MoveTo(sx(game.player.pos.col()), game.player.pos.row() as u16),
        style::PrintStyledContent(ch.to_string().bold().yellow()), // Convert char to string for styling
    )?;
    Ok(())
}

pub fn render_rhs<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
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

    let mode = if game.period() == Period::Chase {
        "chase"
    } else {
        "scatter"
    };

    let i = panel_x();
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
        cursor::MoveTo(i, 11),
        style::PrintStyledContent(format!("Ghosts : {mode:<7}").bold().white()),
    )?;
    draw_message_at(
        w,
        game,
        Position::from_xy(game.board.width - 1, game.board.height),
        get_fruit_symbol(game.bonus()),
    )?;

    let s = vec!['\u{1F642}'; game.lives as usize];
    let s1 = vec![' '; 2 * (MAX_PACMAN_LIVES as usize - s.len())]; // 2 cells per emoji
    let s2: String = s.into_iter().chain(s1).collect::<String>();
    draw_message_at(w, game, Position::from_xy(0, game.board.height), &s2)?;
    draw_marquee(w, game)
}

fn draw_marquee<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
    // scroll marquee
    let (cols, rows) = terminal::size().unwrap_or((80, 24));

    let marquee_x = 0; // start column
    let q: u16 = cols.saturating_sub(1); // Subtract 1 to avoid the "last cell" scroll trigger
    let i1: usize = game.mq_idx % MARQUEE.len();
    let t: usize = q as usize + game.mq_idx;
    let i2: usize = t % MARQUEE.len();

    crossterm::queue!(w, cursor::MoveTo(marquee_x, rows - 1))?;
    if i1 < i2 {
        crossterm::queue!(w, style::PrintStyledContent(MARQUEE[i1..i2].white()))
    } else {
        // marquee is assumed to be ascii (1 byte characters)
        let part1 = &MARQUEE[i1..];
        let part2 = &MARQUEE[0..i2.min(MARQUEE.len())];
        crossterm::queue!(
            w,
            style::PrintStyledContent(format!("{part1}{part2}").white())
        )
    }
}

/// Repaints everything - the terminal has lost or reflowed the screen, e.g. after a resize.
fn redraw(game: &Game) -> io::Result<()> {
    clear_screen()?;
    render_game_info()?;
    draw_dynamic(game)
}

pub fn draw_game_over(game: &Game) -> io::Result<()> {
    let mut w = io::BufWriter::new(stdout());
    draw_message(&mut w, game, "GAME  OVER", true)
}

pub fn clear_screen() -> io::Result<()> {
    crossterm::queue!(stdout(), terminal::Clear(terminal::ClearType::All))
}

pub fn draw_ghosts<W: Write>(w: &mut W, game: &Game) -> io::Result<()> {
    for (i, g) in game.ghosts.iter().enumerate() {
        let s = match (g.state, game.board[g.pos] != Square::House, i) {
            (GhostState::Dead, _, _) => "\u{1F440}",
            (_, true, _) if (1..2000).contains(&g.edible_duration) => "\u{1F47D}",
            (_, true, _) if g.edible_duration > 0 => "\u{1F631}",
            (_, _, 0) => "\u{1F47A}",
            (_, _, 1) => "\u{1F479}",
            (_, _, 2) => "\u{1F47B}",
            (_, _, _) => "\u{1F383}",
        };
        crossterm::queue!(
            w,
            cursor::MoveTo(sx(g.pos.col()), g.pos.row() as u16),
            style::Print(s)
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn panel_lines_fit_the_widest() {
        let widest = WIDEST_INFO.len();
        for level in 0..8 {
            let name = Board::new(level).maze_name;
            assert!(format!("Maze   : {name}").len() <= widest, "{name}");
        }
        assert!(CONTROLS.iter().all(|s| s.len() <= widest));
        assert!("Ghosts : scatter".len() <= widest);
    }

    #[test]
    fn titles_are_centred_over_the_colon() {
        for cols in 80..=200 {
            for s in TITLE {
                let x = over_colon_in(cols, s);
                assert_eq!(
                    x + s.len() as u16 / 2,
                    colon_in(cols),
                    "{s} at {cols} columns"
                );
                assert!(x + s.len() as u16 <= cols);
            }
        }
    }

    #[test]
    fn title_fits_the_panel() {
        // the panel right of the maze, in an 80 column terminal
        let panel = 80 - (sx(WIDTH) as usize + 2);
        assert!(TITLE.iter().all(|s| s.len() <= panel), "{TITLE:?}");
    }
}
