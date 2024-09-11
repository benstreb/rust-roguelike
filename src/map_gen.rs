use rand::{seq::SliceRandom, Rng};
use rusqlite::{types::FromSql, ToSql};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Unused,
    Floor,
    Corridor,
    Wall,
    ClosedDoor,
    OpenDoor,
    UpStairs,
    DownStairs,
    Unknown(char),
}
// pub enum Tile {
// }

impl ToSql for Tile {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        match self {
            Tile::Unused => Ok(" ".into()),
            Tile::Floor => Ok(".".into()),
            Tile::Corridor => Ok(",".into()),
            Tile::Wall => Ok("#".into()),
            Tile::ClosedDoor => Ok("+".into()),
            Tile::OpenDoor => Ok("-".into()),
            Tile::UpStairs => Ok("<".into()),
            Tile::DownStairs => Ok(">".into()),
            Tile::Unknown(c) => Ok(c.to_string().into()),
        }
    }
}

impl FromSql for Tile {
    fn column_result(value: rusqlite::types::ValueRef) -> rusqlite::types::FromSqlResult<Self> {
        let s: String = value.as_str()?.to_string();
        Ok(match s.as_str() {
            " " => Tile::Unused,
            "." => Tile::Floor,
            "," => Tile::Corridor,
            "#" => Tile::Wall,
            "+" => Tile::ClosedDoor,
            "-" => Tile::OpenDoor,
            "<" => Tile::UpStairs,
            ">" => Tile::DownStairs,
            _ => Tile::Unknown(s.chars().next().unwrap()),
        })
    }
}
/*
    class Dungeon {
    private:
        const int _width, _height;
        std::vector<Tile> _tiles;

    private:
        inline Dungeon(int width, int height)
            : _width(width)
            , _height(height)
            , _tiles(width* height, Tile::Unused)
        {
        }
    public:
        static Dungeon generate(pcg32& rng, int width, int height);
        static Dungeon generate_empty(pcg32& rng, int width, int height);
        void foreach_tile(std::function<void(Tile, int, int)> tile_consumer);
        Tile get(int x, int y, Tile _default) const;
        Tile& operator()(int x, int y);

        friend class DefaultGenerator;
    };
*/

pub struct Dungeon {
    width: i64,
    height: i64,
    tiles: Vec<Tile>,
}

impl Dungeon {
    pub fn iter(&self) -> impl Iterator<Item = (Tile, i64, i64)> + '_ {
        let width = self.width;
        self.tiles.iter().enumerate().map(move |(i, &t)| {
            let x = i as i64 % width;
            let y = i as i64 / width;
            (t, x, y)
        })
    }

    fn get(&self, x: i64, y: i64, default: Tile) -> Tile {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            default
        } else {
            self[(x, y)]
        }
    }
}

impl std::ops::Index<(i64, i64)> for Dungeon {
    type Output = Tile;

    fn index(&self, (x, y): (i64, i64)) -> &Self::Output {
        &self.tiles[(x + (y * self.width)) as usize]
    }
}

impl std::ops::IndexMut<(i64, i64)> for Dungeon {
    fn index_mut(&mut self, (x, y): (i64, i64)) -> &mut Self::Output {
        &mut self.tiles[(x + (y * self.width)) as usize]
    }
}

pub trait Generator {
    fn generate(&mut self, rng: &mut rand_pcg::Pcg64Mcg, width: i64, height: i64) -> Dungeon;
}

pub struct EmptyGenerator;

impl Generator for EmptyGenerator {
    fn generate(&mut self, _: &mut rand_pcg::Pcg64Mcg, width: i64, height: i64) -> Dungeon {
        let mut d = Dungeon {
            width: width - 1,
            height: height - 1,
            tiles: vec![Tile::Unused; (width * height) as usize],
        };
        for i in 0..width - 1 {
            for j in 0..height - 1 {
                if i == 0 || i == width - 2 || j == 0 || j == height - 2 {
                    d[(i, j)] = Tile::Wall;
                } else {
                    d[(i, j)] = Tile::Floor;
                }
            }
        }
        d[(1, 1)] = Tile::UpStairs;
        d
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

const MIN_ROOM_SIZE: i64 = 3;
const MAX_ROOM_SIZE: i64 = 6;
const MIN_CORRIDOR_LENGTH: i64 = 3;
const MAX_CORRIDOR_LENGTH: i64 = 6;

pub struct DefaultGenerator {
    rooms: Vec<Rect>,
    exits: Vec<Rect>,
}

impl Generator for DefaultGenerator {
    fn generate(&mut self, rng: &mut rand_pcg::Pcg64Mcg, width: i64, height: i64) -> Dungeon {
        let mut dungeon = Dungeon {
            width: width - 1,
            height: height - 1,
            tiles: vec![Tile::Unused; (width * height) as usize],
        };
        let max_features = 50;
        self.populate(rng, &mut dungeon, max_features);
        dungeon[(1, 1)] = Tile::UpStairs;
        dungeon
    }
}

impl DefaultGenerator {
    pub fn new() -> Self {
        Self {
            rooms: Vec::new(),
            exits: Vec::new(),
        }
    }

    fn populate(&mut self, rng: &mut rand_pcg::Pcg64Mcg, dungeon: &mut Dungeon, max_features: i32) {
        // place the first room in the center
        if !self.make_room(
            rng,
            dungeon.width / 2,
            dungeon.height / 2,
            Direction::North,
            true,
            dungeon,
        ) {
            return;
        }

        // we already placed 1 feature (the first room)
        for i in 1..max_features {
            if !self.create_feature(rng, dungeon) {
                eprintln!("Unable to place more features (placed {}).", i);
                break;
            }
        }

        if !self.place_object(rng, Tile::UpStairs, dungeon) {
            eprintln!("Unable to place up stairs.");
            return;
        }

        if !self.place_object(rng, Tile::DownStairs, dungeon) {
            eprintln!("Unable to place down stairs.");
            return;
        }
    }

    fn create_feature(&mut self, rng: &mut rand_pcg::Pcg64Mcg, dungeon: &mut Dungeon) -> bool {
        let directions = [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ];
        for _ in 0..1000 {
            if self.exits.is_empty() {
                break;
            }

            // choose a random side of a random room or corridor
            let r = rng.gen_range(0..self.exits.len());
            let exit = self.exits[r];
            let x = rng.gen_range(exit.x..exit.x + exit.width);
            let y = rng.gen_range(exit.y..exit.y + exit.height);

            let mut shuffled_directions = directions.to_vec();
            shuffled_directions.shuffle(rng);
            for d in shuffled_directions {
                if self.create_feature_at(rng, x, y, d, dungeon) {
                    self.exits.remove(r);
                    return true;
                }
            }
        }

        false
    }

    fn create_feature_at(
        &mut self,
        rng: &mut rand_pcg::Pcg64Mcg,
        x: i64,
        y: i64,
        dir: Direction,
        dungeon: &mut Dungeon,
    ) -> bool {
        let dx = match dir {
            Direction::North => 0,
            Direction::South => 0,
            Direction::West => 1,
            Direction::East => -1,
        };
        let dy = match dir {
            Direction::North => 1,
            Direction::South => -1,
            Direction::West => 0,
            Direction::East => 0,
        };

        let candidate = dungeon.get(x + dx, y + dy, Tile::Unused);
        if candidate != Tile::Floor && candidate != Tile::Corridor {
            return false;
        }

        let room_chance = 50; // corridor_chance = 100 - room_chance

        if rng.gen_range(0..100) < room_chance {
            if self.make_room(rng, x, y, dir, false, dungeon) {
                dungeon[(x, y)] = Tile::ClosedDoor;
                return true;
            }
        } else {
            if self.make_corridor(rng, x, y, dir, dungeon) {
                if candidate == Tile::Floor {
                    dungeon[(x, y)] = Tile::ClosedDoor;
                } else {
                    dungeon[(x, y)] = Tile::Corridor;
                }
                return true;
            }
        }

        false
    }

    fn make_room(
        &mut self,
        rng: &mut rand_pcg::Pcg64Mcg,
        x: i64,
        y: i64,
        dir: Direction,
        first_room: bool,
        dungeon: &mut Dungeon,
    ) -> bool {
        let room = self.random_room(rng, (x, y), dir);

        if self.place_rect(room, Tile::Floor, dungeon) {
            self.rooms.push(room);

            if dir != Direction::South || first_room {
                // north side
                self.exits.push(Rect {
                    x: room.x,
                    y: room.y - 1,
                    width: room.width,
                    height: 1,
                });
            }
            if dir != Direction::North || first_room {
                // south side
                self.exits.push(Rect {
                    x: room.x,
                    y: room.y + room.height,
                    width: room.width,
                    height: 1,
                });
            }
            if dir != Direction::East || first_room {
                // west side
                self.exits.push(Rect {
                    x: room.x - 1,
                    y: room.y,
                    width: 1,
                    height: room.height,
                });
            }
            if dir != Direction::West || first_room {
                // east side
                self.exits.push(Rect {
                    x: room.x + room.width,
                    y: room.y,
                    width: 1,
                    height: room.height,
                });
            }

            true
        } else {
            false
        }
    }

    fn make_corridor(
        &mut self,
        rng: &mut rand_pcg::Pcg64Mcg,
        x: i64,
        y: i64,
        dir: Direction,
        dungeon: &mut Dungeon,
    ) -> bool {
        let corridor = self.random_corridor(rng, (x, y), dir);

        if self.place_rect(corridor, Tile::Corridor, dungeon) {
            if dir != Direction::South && corridor.width != 1 {
                // north side
                self.exits.push(Rect {
                    x: corridor.x,
                    y: corridor.y - 1,
                    width: corridor.width,
                    height: 1,
                });
            }
            if dir != Direction::North && corridor.width != 1 {
                // south side
                self.exits.push(Rect {
                    x: corridor.x,
                    y: corridor.y + corridor.height,
                    width: corridor.width,
                    height: 1,
                });
            }
            if dir != Direction::East && corridor.height != 1 {
                // west side
                self.exits.push(Rect {
                    x: corridor.x - 1,
                    y: corridor.y,
                    width: 1,
                    height: corridor.height,
                });
            }
            if dir != Direction::West && corridor.height != 1 {
                // east side
                self.exits.push(Rect {
                    x: corridor.x + corridor.width,
                    y: corridor.y,
                    width: 1,
                    height: corridor.height,
                });
            }

            true
        } else {
            false
        }
    }

    fn place_rect(&mut self, rect: Rect, tile: Tile, dungeon: &mut Dungeon) -> bool {
        if rect.x < 1
            || rect.y < 1
            || rect.x + rect.width > dungeon.width - 1
            || rect.y + rect.height > dungeon.height - 1
        {
            return false;
        }

        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                if dungeon.get(x, y, Tile::Unused) != Tile::Unused {
                    return false; // the area already used
                }
            }
        }

        for y in rect.y - 1..rect.y + rect.height + 1 {
            for x in rect.x - 1..rect.x + rect.width + 1 {
                if x == rect.x - 1
                    || y == rect.y - 1
                    || x == rect.x + rect.width
                    || y == rect.y + rect.height
                {
                    dungeon[(x, y)] = Tile::Wall;
                } else {
                    dungeon[(x, y)] = tile;
                }
            }
        }

        true
    }

    fn place_object(
        &mut self,
        rng: &mut rand_pcg::Pcg64Mcg,
        tile: Tile,
        dungeon: &mut Dungeon,
    ) -> bool {
        if self.rooms.is_empty() {
            return false;
        }

        let r = rng.gen_range(0..self.rooms.len()); // choose a random room
        let room = self.rooms[r];
        let x = rng.gen_range(room.x + 1..room.x + room.width - 1);
        let y = rng.gen_range(room.y + 1..room.y + room.height - 1);

        if dungeon[(x, y)] == Tile::Floor {
            dungeon[(x, y)] = tile;
            self.rooms.remove(r);
            true
        } else {
            false
        }
    }

    fn random_room(
        &self,
        rng: &mut rand_pcg::Pcg64Mcg,
        anchor: (i64, i64),
        dir: Direction,
    ) -> Rect {
        let (x, y) = anchor;

        let mut room = Rect {
            x: 0,
            y: 0,
            width: rng.gen_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE),
            height: rng.gen_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE),
        };

        match dir {
            Direction::North => {
                room.x = x - room.width / 2;
                room.y = y - room.height;
            }
            Direction::South => {
                room.x = x - room.width / 2;
                room.y = y + 1;
            }
            Direction::West => {
                room.x = x - room.width;
                room.y = y - room.height / 2;
            }
            Direction::East => {
                room.x = x + 1;
                room.y = y - room.height / 2;
            }
        }

        room
    }

    fn random_corridor(
        &self,
        rng: &mut rand_pcg::Pcg64Mcg,
        anchor: (i64, i64),
        dir: Direction,
    ) -> Rect {
        let (x, y) = anchor;

        let mut corridor = Rect {
            x,
            y,
            width: 0,
            height: 0,
        };

        if rng.gen_bool(0.5) {
            // horizontal corridor
            corridor.width = rng.gen_range(MIN_CORRIDOR_LENGTH..=MAX_CORRIDOR_LENGTH);
            corridor.height = 1;

            match dir {
                Direction::North => {
                    corridor.y = y - 1;
                    if rng.gen_bool(0.5) {
                        // west
                        corridor.x = x - corridor.width + 1;
                    }
                }
                Direction::South => {
                    corridor.y = y + 1;
                    if rng.gen_bool(0.5) {
                        // west
                        corridor.x = x - corridor.width + 1;
                    }
                }
                Direction::West => {
                    corridor.x = x - corridor.width;
                }
                Direction::East => {
                    corridor.x = x + 1;
                }
            }
        } else {
            // vertical corridor
            corridor.width = 1;
            corridor.height = rng.gen_range(MIN_CORRIDOR_LENGTH..=MAX_CORRIDOR_LENGTH);

            match dir {
                Direction::North => {
                    corridor.y = y - corridor.height;
                }
                Direction::South => {
                    corridor.y = y + 1;
                }
                Direction::West => {
                    corridor.x = x - 1;
                    if rng.gen_bool(0.5) {
                        // north
                        corridor.y = y - corridor.height + 1;
                    }
                }
                Direction::East => {
                    corridor.x = x + 1;
                    if rng.gen_bool(0.5) {
                        // north
                        corridor.y = y - corridor.height + 1;
                    }
                }
            }
        }

        corridor
    }
}
