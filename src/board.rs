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

static LEVEL1MAP: [&str; 29] = [
    "############################", //  0
    "#............##............#", //  1
    "#.####.#####.##.#####.####.#", //  2
    "#P####.#####.##.#####.####P#", //  3
    "#.####.#####.##.#####.####.#", //  4
    "#..........................#", //  5
    "#.####.##.########.##.####.#", //  6
    "#.####.##.########.##.####.#", //  7
    "#......##....##....##......#", //  8
    "######.##### ## #####.######", //  9
    "     #.##          ##.#     ", // 10
    "     #.## ###--### ##.#     ", // 11
    "######.## # HHHH # ##.######", // 12
    ";;;;;;.   # HHHH #   .;;;;;;", // 13
    "######.## # HHHH # ##.######", // 14
    "     #.## ######## ##.#     ", // 15
    "     #.##    $     ##.#     ", // 16
    "######.## ######## ##.######", // 17
    "#............##............#", // 18
    "#.####.#####.##.#####.####.#", // 19
    "#.####.#####.##.#####.####.#", // 20
    "#P..##.......p........##..P#", // 21
    "###.##.##.########.##.##.###", // 22
    "###.##.##.########.##.##.###", // 23
    "#......##....##....##......#", // 24
    "#.##########.##.##########.#", // 25
    "#.##########.##.##########.#", // 26
    "#..........................#", // 27
    "############################", // 28
];

static LEVEL1MAPc: [&str; 24] = [
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
    "#P..##.......p........##..P#", // 18
    "###.##.##.########.##.##.###", // 19
    "#......##....##....##......#", // 20
    "#.##########.##.##########.#", // 21
    "#..........................#", // 22
    "############################", // 23
];

pub const WIDTH: usize = LEVEL1MAP[0].len();
pub const HEIGHT: usize = LEVEL1MAP.len();

pub struct Board {
    board: [char; WIDTH * HEIGHT],
    pub gate1: Position,
    pub gate2: Position,
    pub front_of_gate1: Position,
    pub front_of_gate2: Position,
    pub fruit_pos: Position,
    pub pacman_start: Position,
    pub ghost_start: [Position; 4],
}

impl Board {
    pub fn new(_level: u32) -> Self {
        let board_chars: Vec<char> = LEVEL1MAP.iter().flat_map(|&s| s.chars()).collect();
        let board: [char; WIDTH * HEIGHT] = board_chars.try_into().expect("Board size mismatch");

        // board must have:
        // * two ghost gate positions: '-' (north exit)
        // * a fruit bonus location: '$'
        // * a start position for pacman: 'p'
        let gate1 = Position(
            board
                .iter()
                .position(|c| *c == '-')
                .expect("no ghost gate on map"),
        );
        let gate2 = Position(
            gate1.0
                + 1
                + board[gate1.0 + 1..]
                    .iter()
                    .position(|c| *c == '-')
                    .expect("only one ghost gate on map"),
        );
        let fruit_pos = Position(
            board
                .iter()
                .position(|c| *c == '$')
                .expect("no bonus fruit on map"),
        );

        let pacman_start = Position(
            board
                .iter()
                .position(|c| *c == 'p')
                .expect("no start position for pacman"),
        );
        let ghost_house: Vec<Position> = board
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == 'H')
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
            gate1,
            gate2,
            pacman_start,
            front_of_gate1: gate1.go(Up),
            front_of_gate2: gate2.go(Up),
            fruit_pos,
            ghost_start,
        }
    }

    pub fn dots(&self) -> usize {
        self.board.iter().filter(|&c| *c == '.').count()
    }
}

impl Index<Position> for Board {
    type Output = char;
    fn index(&self, idx: Position) -> &Self::Output {
        &self.board[idx.0]
    }
}

impl IndexMut<Position> for Board {
    fn index_mut(&mut self, idx: Position) -> &mut Self::Output {
        &mut self.board[idx.0]
    }
}
