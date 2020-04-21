use serde::{Deserialize, Serialize};
use std::convert::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Position {
    pub row: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
#[serde(from = "[u32; 4]", into = "[u32; 4]")]
pub struct Location {
    pub start: Position,
    pub end: Position,
}

impl Location {
    pub fn new(sr: u32, sc: u32, er: u32, ec: u32) -> Location {
        Location {
            start: Position { row: sr, col: sc },
            end: Position { row: er, col: ec },
        }
    }
}

impl From<Location> for [u32; 4] {
    fn from(l: Location) -> Self {
        [l.start.row, l.start.col, l.end.row, l.end.col]
    }
}

impl From<[u32; 4]> for Location {
    fn from(array: [u32; 4]) -> Self {
        Location {
            start: Position {
                row: array[0],
                col: array[1],
            },
            end: Position {
                row: array[2],
                col: array[3],
            },
        }
    }
}
