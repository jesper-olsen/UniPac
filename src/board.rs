use crate::maze::*;
use std::ops::{Index, IndexMut};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Position(usize);

impl Position {
    pub const fn from_xy(col: usize, row: usize) -> Position {
        Position(row * WIDTH + col)
    }

    pub const fn col(&self) -> usize {
        self.0 % WIDTH
    }

    pub const fn row(&self) -> usize {
        self.0 / WIDTH
    }

    pub const fn dist_city(&self, other: Position) -> usize {
        self.col().abs_diff(other.col()) + self.row().abs_diff(other.row())
    }

    pub const fn dist_sqr(&self, other: Position) -> usize {
        self.col().abs_diff(other.col()).pow(2) + self.row().abs_diff(other.row()).pow(2)
    }

    pub const fn average(&self, other: Position) -> Position {
        Position::from_xy(
            (self.col() + other.col()) / 2,
            (self.row() + other.row()) / 2,
        )
    }

    pub const fn go(&self, direction: Direction) -> Position {
        match direction {
            Right if self.col() == WIDTH - 1 => Position(self.0 - (WIDTH - 1)),
            Right => Position(self.0 + 1),
            Left if self.col() == 0 => Position(self.0 + (WIDTH - 1)),
            Left => Position(self.0 - 1),
            Down => Position(self.0 + WIDTH),
            Up => Position(self.0 - WIDTH),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Square {
    Empty,
    Dot,
    Pill,
    Fruit,
    Start,
    Wall,
    Gate,
    House,
    Tunnel,
}

impl Square {
    pub fn from_char(ch: char) -> Square {
        match ch {
            ' ' => Square::Empty,
            '.' => Square::Dot,
            'P' => Square::Pill,
            '$' => Square::Fruit,
            'p' => Square::Start,
            '#' => Square::Wall,
            '-' => Square::Gate,
            'H' => Square::House,
            ';' => Square::Tunnel,
            _ => panic!("not a valid maze symbol: {ch}"),
        }
    }
}

pub struct Board {
    board: Vec<Square>,
    pub maze_name: &'static str,
    pub width: usize,
    pub height: usize,
    pub gate1: Position,
    pub gate2: Position,
    pub front_of_gate1: Position,
    pub front_of_gate2: Position,
    pub fruit: Position,
    pub pacman_start: Position,
    pub ghost_start: [Position; 4],
}

impl Board {
    pub fn new(level: u32) -> Self {
        let (maze, maze_name): (&[&str], &str) = match level {
            0 => (&MAZE_SMALL_PACMAN, "Pacman Small"),
            2 => (&MAZE_MS_PACMAN_PINK, "Ms. Pacman Pink"),
            3 => (&MAZE_MS_PACMAN_LIGHT_BLUE, "Ms. Pacman Light Blue"),
            4 => (&MAZE_MS_PACMAN_ORANGE, "Ms. Pacman Orange"),
            5 => (&MAZE_MS_PACMAN_DARK_BLUE, "Ms. Pacman Dark Blue"),
            _ => (&MAZE_REG_PACMAN, "Pacman Regular"),
        };
        let board: Vec<Square> = maze
            .iter()
            .flat_map(|&row| row.chars().map(Square::from_char))
            .collect();
        let width = maze[0].len();
        let height = board.len() / width;
        if width != WIDTH {
            panic!("Maze has wrong width {width} expected {WIDTH}")
        }

        // board must have:
        // * two ghost gate positions: '-' (north exit)
        // * a fruit bonus location: '$'
        // * a start position for pacman: 'p'
        let gate1 = Position(
            board
                .iter()
                .position(|c| *c == Square::Gate)
                .expect("no ghost gate on map"),
        );
        let gate2 = Position(
            gate1.0
                + 1
                + board[gate1.0 + 1..]
                    .iter()
                    .position(|c| *c == Square::Gate)
                    .expect("only one ghost gate on map"),
        );
        let fruit = Position(
            board
                .iter()
                .position(|c| *c == Square::Fruit)
                .expect("no bonus fruit on map"),
        );

        let pacman_start = Position(
            board
                .iter()
                .position(|c| *c == Square::Start)
                .expect("no start position for pacman"),
        );
        let ghost_house: Vec<Position> = board
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == Square::House)
            .map(|(i, _)| Position(i))
            .collect();
        let min_col = ghost_house
            .iter()
            .map(|p| p.col())
            .min()
            .expect("no ghost house");
        let max_col = ghost_house
            .iter()
            .map(|p| p.col())
            .max()
            .expect("no ghost house");
        let min_row = ghost_house
            .iter()
            .map(|p| p.row())
            .min()
            .expect("no ghost house");
        let max_row = ghost_house
            .iter()
            .map(|p| p.row())
            .max()
            .expect("no ghost house");
        let ghost_start = [
            Position::from_xy(min_col, min_row),
            Position::from_xy(max_col, min_row),
            Position::from_xy(min_col, max_row),
            Position::from_xy(max_col, max_row),
        ];
        Board {
            board,
            maze_name,
            width,
            height,
            gate1,
            gate2,
            pacman_start,
            front_of_gate1: gate1.go(Up),
            front_of_gate2: gate2.go(Up),
            fruit,
            ghost_start,
        }
    }

    pub fn dots(&self) -> usize {
        self.board.iter().filter(|&c| *c == Square::Dot).count()
    }
}

impl Index<Position> for Board {
    type Output = Square;
    fn index(&self, idx: Position) -> &Self::Output {
        &self.board[idx.0]
    }
}

impl IndexMut<Position> for Board {
    fn index_mut(&mut self, idx: Position) -> &mut Self::Output {
        &mut self.board[idx.0]
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_eval() {
        for i in 0..4 {
            let _ = Board::new(i);
        }
    }
}
