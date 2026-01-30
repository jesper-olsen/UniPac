use crate::{
    Game, GhostState, MARQUEE, MAX_PACMAN_LIVES, Period, Position,
    board::{Direction, Square},
};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, poll, read},
    style::{self, Stylize},
    terminal,
};
use std::io::{self, Write, stdout};
use std::time::Duration;
use std::{thread, time};

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

pub fn centered_x(s: &str) -> u16 {
    let leftedge: u16 = 32;
    let cols = terminal::size().map(|(c, _)| c).unwrap_or(80);
    let n: u16 = s.len() as u16;
    let offset = cols.saturating_sub(leftedge).saturating_sub(n) / 2;
    offset + leftedge
}

pub fn draw_message<W: Write>(w: &mut W, game: &Game, s: &str, blink: bool) -> io::Result<()> {
    let col = ((game.board.width - s.len()) / 2) as u16;
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
        std::cmp::min(pos.col(), game.board.width - 4) as u16,
        pos.row() as u16,
    );
    crossterm::queue!(
        w,
        cursor::MoveTo(col, row),
        style::PrintStyledContent(s.bold())
    )?;
    stdout().flush()
}

//  The animated death and flashing screen happen syncronously. To be done
//  correctly, they should be pseudo-event driven like the rest of the program.
pub fn draw_dynamic(game: &Game) -> io::Result<()> {
    let mut w = io::BufWriter::new(stdout());
    draw_board(&mut w, game, false)?;
    draw_player(&mut w, game)?;
    draw_ghosts(&mut w, game)?;
    render_rhs(&mut w, game)?;
    w.flush()
}

pub fn draw_board<W: Write>(w: &mut W, game: &Game, bold: bool) -> io::Result<()> {
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
        let fruit = game.bonus().0;
        let (col, row) = (game.board.fruit.col(), game.board.fruit.row());
        crossterm::queue!(
            w,
            cursor::MoveTo(col as u16, row as u16),
            style::Print(fruit),
        )?;
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
    let (col, row) = (game.player.pos.col() as u16, game.player.pos.row() as u16);
    crossterm::queue!(
        w,
        cursor::MoveTo(col, row),
        style::PrintStyledContent(ch.bold().yellow())
    )
}

pub enum InputEvent {
    Direction(Direction),
    Quit,
    Pause,
    Cheat,
    None,
}

pub fn poll_input() -> io::Result<InputEvent> {
    if poll(Duration::from_millis(10))?
        && let Event::Key(key_event) = read()?
        && key_event.kind == event::KeyEventKind::Press
    {
        return Ok(match key_event.code {
            KeyCode::Char('q') => InputEvent::Quit,
            KeyCode::Char('v') => InputEvent::Cheat,
            KeyCode::Char(' ') => InputEvent::Pause,
            KeyCode::Left => InputEvent::Direction(Direction::Left),
            KeyCode::Right => InputEvent::Direction(Direction::Right),
            KeyCode::Up => InputEvent::Direction(Direction::Up),
            KeyCode::Down => InputEvent::Direction(Direction::Down),
            _ => InputEvent::None,
        });
    }
    Ok(InputEvent::None)
}

pub fn pause(game: &Game) -> io::Result<()> {
    let mut w = io::BufWriter::new(stdout());
    draw_message(&mut w, game, "PAUSED", false)?;
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

pub fn another_game() -> io::Result<bool> {
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

pub fn render_game_info() -> io::Result<()> {
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

pub fn animate_dead_player(game: &Game) -> io::Result<()> {
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
    draw_message_at(
        w,
        game,
        Position::from_xy(game.board.width - 1, game.board.height),
        game.bonus().0,
    )?;

    let s = vec!['\u{1F642}'; game.lives as usize];
    let s1 = vec![' '; MAX_PACMAN_LIVES as usize - s.len()];
    let s2: String = s.into_iter().chain(s1).collect::<String>();
    draw_message_at(w, game, Position::from_xy(0, game.board.height), &s2)?;

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

pub fn flash_board(game: &Game) -> io::Result<()> {
    for i in 0..10 {
        let mut w = io::BufWriter::new(stdout());
        draw_board(&mut w, game, i % 2 == 0)?;
        w.flush()?;
        thread::sleep(time::Duration::from_millis(300));
    }
    // clear screen - next board may have different height
    crossterm::queue!(stdout(), terminal::Clear(terminal::ClearType::All),)?;
    Ok(())
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
            cursor::MoveTo(g.pos.col() as u16, g.pos.row() as u16),
            style::Print(s)
        )?;
    }
    Ok(())
}
