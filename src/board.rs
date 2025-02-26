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

static LEVEL1MAP: [&str; 24] = [
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
    ";;;;;;.   # HHHH #   .;;;;;;", // 11
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
];

pub const WIDTH: usize = LEVEL1MAP[0].len();
pub const HEIGHT: usize = LEVEL1MAP.len();

pub struct Board([char; WIDTH * HEIGHT]);

impl Board {
    pub fn new(_level: usize) -> Self {
        let board_chars: Vec<char> = LEVEL1MAP.iter().flat_map(|&s| s.chars()).collect();
        let board_array: [char; WIDTH * HEIGHT] =
            board_chars.try_into().expect("Board size mismatch");
        Board(board_array)
    }

    pub fn dots(&self) -> usize {
        self.0.iter().filter(|&c| *c == '.').count()
    }
}

impl Index<Position> for Board {
    type Output = char;
    fn index(&self, idx: Position) -> &Self::Output {
        &self.0[idx.0]
    }
}

impl IndexMut<Position> for Board {
    fn index_mut(&mut self, idx: Position) -> &mut Self::Output {
        &mut self.0[idx.0]
    }
}
