use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    pub row: u32,
    pub col: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

impl Serialize for Location {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let array = [self.start.row, self.start.col, self.end.row, self.end.col];
        array.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Location {
    fn deserialize<D>(deserializer: D) -> Result<Location, D::Error>
    where
        D: Deserializer<'de>,
    {
        let array = <[u32; 4] as Deserialize<'de>>::deserialize(deserializer)?;
        Ok(Location {
            start: Position {
                row: array[0],
                col: array[1],
            },
            end: Position {
                row: array[2],
                col: array[3],
            },
        })
    }
}
