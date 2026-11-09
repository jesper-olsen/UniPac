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

    pub const fn dist_sqr(&self, other: Position) -> usize {
        self.col().abs_diff(other.col()).pow(2) + self.row().abs_diff(other.row()).pow(2)
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

impl TryFrom<char> for Square {
    type Error = String;

    fn try_from(ch: char) -> Result<Self, Self::Error> {
        match ch {
            ' ' => Ok(Square::Empty),
            '.' => Ok(Square::Dot),
            'P' => Ok(Square::Pill),
            '$' => Ok(Square::Fruit),
            'p' => Ok(Square::Start),
            '#' => Ok(Square::Wall),
            '-' => Ok(Square::Gate),
            'H' => Ok(Square::House),
            ';' => Ok(Square::Tunnel),
            _ => Err(format!("Invalid maze symbol: {ch}")),
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
    pub ghost_start: [Position; 4], // Pinky, Blinky, Inky, Clyde
}

impl Board {
    pub fn new(level: u32) -> Self {
        let (maze, maze_name): (&[&str], &str) = match level {
            0 => (&MAZE_SMALL_PACMAN, "Small"),
            2 => (&MAZE_MS_PACMAN_PINK, "Pink"),
            3 => (&MAZE_MS_PACMAN_LIGHT_BLUE, "Light Blue"),
            4 => (&MAZE_MS_PACMAN_ORANGE, "Orange"),
            5 => (&MAZE_MS_PACMAN_DARK_BLUE, "Dark Blue"),
            _ => (&MAZE_REG_PACMAN, "Regular"),
        };

        let board: Vec<Square> = maze
            .iter()
            .flat_map(|&row| row.chars())
            .map(Square::try_from)
            .collect::<Result<Vec<Square>, String>>()
            .expect("Maze initialization failed");

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
        let (min_row, max_row) = board
            .iter()
            .enumerate()
            .filter(|&(_, sq)| *sq == Square::House)
            .map(|(i, _)| Position(i).row())
            .fold((usize::MAX, 0), |(lo, hi), r| (lo.min(r), hi.max(r)));
        assert!(min_row <= max_row, "no ghost house");

        // As in the arcade game: Blinky starts outside, above the gate. The others start
        // inside, in the middle row: Pinky below the gate, Inky two tiles to the left and
        // Clyde two to the right.
        let middle = (min_row + max_row) / 2;
        let ghost_start = [
            Position::from_xy(gate1.col(), middle),     // Pinky
            gate1.go(Up),                               // Blinky
            Position::from_xy(gate1.col() - 2, middle), // Inky
            Position::from_xy(gate1.col() + 2, middle), // Clyde
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

    #[test]
    fn ghost_house_starts() {
        for level in 0..7 {
            let b = Board::new(level);
            let [pinky, blinky, inky, clyde] = b.ghost_start;
            assert_eq!(
                b[blinky],
                Square::Empty,
                "Blinky starts outside, level {level}"
            );
            for g in [pinky, inky, clyde] {
                // inside the house: the tiles between the gate and the walls, in one row
                assert!(
                    matches!(b[g], Square::House | Square::Empty),
                    "level {level}"
                );
                assert_eq!(g.row(), pinky.row());
            }
            // straight up from Pinky is the gate, through house tiles only (2 or 3 rows high)
            let mut p = pinky;
            loop {
                p = p.go(Up);
                match b[p] {
                    Square::House => (),
                    Square::Gate => break,
                    sq => panic!("level {level}: unexpected {sq:?} above Pinky"),
                }
            }
            assert!(p == b.gate1);
            // Inky and Clyde can walk to the gate column along their row
            let (lo, hi) = (inky.col(), clyde.col());
            for c in lo..=hi {
                let t = Position::from_xy(c, pinky.row());
                assert!(
                    matches!(b[t], Square::House | Square::Empty),
                    "level {level} col {c}"
                );
            }
        }
    }
}
