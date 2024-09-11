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
    fn generate(&self, rng: &mut rand_pcg::Pcg64Mcg, width: i64, height: i64) -> Dungeon;
}

pub struct EmptyGenerator;

impl Generator for EmptyGenerator {
    fn generate(&self, _: &mut rand_pcg::Pcg64Mcg, width: i64, height: i64) -> Dungeon {
        let mut d = Dungeon {
            width: width - 1,
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
