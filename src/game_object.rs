use crate::{component, entity};
use bracket_lib::color;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

pub const CONSOLE_WIDTH: i64 = 80;
pub const CONSOLE_HEIGHT: i64 = 25;

pub const WIN_LEVEL: &str = "win";

#[derive(Debug, PartialEq, Eq)]
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

impl From<Color> for color::RGBA {
    fn from(value: Color) -> Self {
        color::RGBA {
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

#[derive(Debug)]
pub struct MenuColor {
    pub fg: Color,
    pub bg: Color,
}

pub const MENU_COLOR_UNSELECTED: MenuColor = MenuColor {
    fg: Color::from_u8s(color::WHITE),
    bg: Color::from_u8s(color::BLACK),
};

pub const MENU_COLOR_SELECTED: MenuColor = MenuColor {
    fg: Color::from_u8s(color::BLACK),
    bg: Color::from_u8s(color::WHITE),
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

pub fn init_player(db: &rusqlite::Connection) -> rusqlite::Result<entity::Entity> {
    let player = entity::create(db)?;
    component::player::set(db, player)?;
    component::velocity::set(db, player, 0, 0)?;
    component::collision::set(db, player, false, true, false)?;
    Ok(player)
}

pub fn init_floor(db: &rusqlite::Connection, x: i64, y: i64) -> rusqlite::Result<entity::Entity> {
    let panel = entity::create(db)?;
    component::actor::set(db, panel, ".", x, y, GROUND_COLOR.into(), Plane::Ground)?;
    component::collision::set(db, panel, true, false, false)?;
    Ok(panel)
}

pub fn init_wall(
    db: &rusqlite::Connection,
    tile: &str,
    x: i64,
    y: i64,
) -> rusqlite::Result<entity::Entity> {
    let panel = entity::create(db)?;
    component::actor::set(db, panel, tile, x, y, WALL_COLOR.into(), Plane::Wall)?;
    component::collision::set(db, panel, true, true, false)?;
    Ok(panel)
}

pub fn generate_particles(db: &rusqlite::Connection, lifespan: i64) -> rusqlite::Result<()> {
    let entity = entity::create(db)?;
    component::actor::set_on_random_empty_ground(
        db,
        entity,
        "*",
        PARTICLE_COLOR.into(),
        Plane::Particles,
    )?;
    component::velocity::set_random(db, entity, -1..=1)?;
    component::health::set(db, entity, lifespan, lifespan, -1)?;
    component::collision::set(db, entity, false, false, true)?;
    Ok(())
}

pub fn generate_enemies(db: &rusqlite::Connection, lifespan: i64) -> rusqlite::Result<()> {
    let entity = entity::create(db).unwrap();
    component::actor::set_on_random_empty_ground(
        db,
        entity,
        "x",
        ENEMY_COLOR.into(),
        Plane::Enemies,
    )?;
    component::velocity::set(db, entity, 0, 0)?;
    component::health::set(db, entity, lifespan, lifespan, -1)?;
    component::collision::set(db, entity, false, true, false)?;
    component::ai::set_target_player(db, entity)?;
    // component::ai::set_random(sql, entity)?;
    Ok(())
}
