use crate::{
    component::{self, spawn_attempt::SpawnAttempt},
    entity,
};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

pub const WIN_LEVEL: &str = "win";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn delta(&self) -> (i64, i64) {
        match self {
            Direction::West => (-1, 0),
            Direction::East => (1, 0),
            Direction::North => (0, -1),
            Direction::South => (0, 1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldPoint {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    const fn from_u8s(c: (u8, u8, u8)) -> Color {
        let (r, g, b) = c;
        Color { r, g, b }
    }
}

impl From<Color> for ggez::graphics::Color {
    fn from(value: Color) -> Self {
        ggez::graphics::Color {
            r: value.r as f32 / 255.0,
            g: value.r as f32 / 255.0,
            b: value.r as f32 / 255.0,
            a: 1.0,
        }
    }
}

pub const GROUND_COLOR: Color = Color::from_u8s((80, 80, 80));
pub const PARTICLE_COLOR: Color = Color::from_u8s((200, 200, 200));
pub const ENEMY_COLOR: Color = Color::from_u8s((255, 255, 255));
pub const PLAYER_COLOR: Color = Color::from_u8s((255, 255, 255));
pub const WALL_COLOR: Color = Color::from_u8s((255, 255, 255));
pub const STAIR_COLOR: Color = Color::from_u8s((255, 255, 255));
pub const BACKGROUND_COLOR: Color = Color::from_u8s((0, 0, 0));

#[derive(Debug)]
pub struct ColorPair {
    pub fg: Color,
    pub bg: Color,
}

pub const MENU_COLOR_UNSELECTED: ColorPair = ColorPair {
    fg: Color::from_u8s((255, 255, 255)),
    bg: Color::from_u8s((0, 0, 0)),
};

pub const MENU_COLOR_SELECTED: ColorPair = ColorPair {
    fg: Color::from_u8s((0, 0, 0)),
    bg: Color::from_u8s((255, 255, 255)),
};

#[derive(Clone, Copy, Debug, num_enum::TryFromPrimitive)]
#[repr(i64)]
pub enum Plane {
    Player = 0,
    Enemies = 5,
    Particles = 10,
    Objects = 90,
    Wall = 99,
    Ground = 100,
}

impl ToSql for Plane {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok((*self as i64).into())
    }
}

impl FromSql for Plane {
    fn column_result(value: ValueRef) -> FromSqlResult<Self> {
        Plane::try_from(value.as_i64()?).map_err(|_| FromSqlError::InvalidType)
    }
}

#[derive(Debug)]
pub struct Object {
    pub pos: WorldPoint,
    pub tile: component::tile::Tile,
}

pub fn init_player(
    db: &rusqlite::Connection,
    is_creative: bool,
) -> rusqlite::Result<entity::Entity> {
    let player = entity::create(db)?;
    component::tile::set(
        &db,
        component::tile::Tile {
            entity: player,
            icon: "@".into(),
            color: STAIR_COLOR,
            plane: Plane::Player,
        },
    )?;
    component::player::set(db, player, is_creative)?;
    component::velocity::set(db, player, 0, 0)?;
    component::collision::set(db, player, false, true, false)?;
    component::temperature::set_floating(db, player, 36)?;
    component::temperature::set_heat_source(db, player, 1)?;
    Ok(player)
}

pub fn init_floor(db: &rusqlite::Connection, pos: WorldPoint) -> rusqlite::Result<entity::Entity> {
    let panel = entity::create(db)?;
    component::actor::set(db, panel, pos)?;
    component::tile::set(
        &db,
        component::tile::Tile {
            entity: panel,
            icon: ".".into(),
            color: GROUND_COLOR,
            plane: Plane::Ground,
        },
    )?;
    component::collision::set(db, panel, true, false, false)?;
    component::temperature::set_floating(db, panel, 24)?;
    Ok(panel)
}

pub fn init_wall(
    db: &rusqlite::Connection,
    tile: &str,
    pos: WorldPoint,
) -> rusqlite::Result<entity::Entity> {
    let panel = entity::create(db)?;
    component::actor::set(db, panel, pos)?;
    component::tile::set(
        &db,
        component::tile::Tile {
            entity: panel,
            icon: tile.into(),
            color: WALL_COLOR,
            plane: Plane::Wall,
        },
    )?;
    component::collision::set(db, panel, true, true, false)?;
    component::temperature::set_fixed(db, panel, 24)?;
    Ok(panel)
}

pub fn init_downstairs(db: &rusqlite::Connection, pos: WorldPoint) -> rusqlite::Result<()> {
    let down_stairs = entity::create(&db)?;
    component::actor::set(&db, down_stairs, pos)?;
    component::tile::set(
        &db,
        component::tile::Tile {
            entity: down_stairs,
            icon: ">".into(),
            color: PLAYER_COLOR,
            plane: Plane::Objects,
        },
    )?;
    component::transition::set(&db, down_stairs, WIN_LEVEL)?;
    Ok(())
}

pub fn generate_particles(
    db: &rusqlite::Connection,
    lifespan: i64,
    attempt: SpawnAttempt,
) -> rusqlite::Result<()> {
    let entity = entity::create(db)?;
    component::spawn_attempt::create(db, entity, attempt)?;
    component::tile::set(
        &db,
        component::tile::Tile {
            entity,
            icon: "*".into(),
            color: PARTICLE_COLOR,
            plane: Plane::Particles,
        },
    )?;
    component::velocity::set_random(db, entity, -1..=1)?;
    component::health::set(db, entity, lifespan, lifespan, -1)?;
    component::collision::set(db, entity, false, false, true)?;
    Ok(())
}

pub fn generate_enemies(db: &rusqlite::Connection, lifespan: i64) -> rusqlite::Result<()> {
    let entity = entity::create(db).unwrap();
    component::spawn_attempt::create(db, entity, SpawnAttempt::AnyUnoccupied)?;
    component::tile::set(
        &db,
        component::tile::Tile {
            entity,
            icon: "x".into(),
            color: ENEMY_COLOR,
            plane: Plane::Enemies,
        },
    )?;
    component::velocity::set(db, entity, 0, 0)?;
    component::health::set(db, entity, lifespan, lifespan, -1)?;
    component::collision::set(db, entity, false, true, false)?;
    component::ai::set_random(db, entity)?;
    component::temperature::set_floating(db, entity, 94)?;
    component::temperature::set_heat_source(db, entity, 1)?;
    Ok(())
}
