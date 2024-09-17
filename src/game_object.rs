use crate::{component, entity};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

pub const CONSOLE_WIDTH: i64 = 80;
pub const CONSOLE_HEIGHT: i64 = 25;

pub const WIN_LEVEL: &str = "win";

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

pub fn init_player(sql: &rusqlite::Connection) -> rusqlite::Result<entity::Entity> {
    let player = entity::create(sql)?;
    component::player::set(sql, player)?;
    component::velocity::set(sql, player, 0, 0)?;
    component::collision::set(sql, player, false, true, false)?;
    Ok(player)
}

pub fn init_floor(sql: &rusqlite::Connection, x: i64, y: i64) -> rusqlite::Result<entity::Entity> {
    let panel = entity::create(sql)?;
    component::actor::set(sql, panel, ".", x, y, Plane::Ground)?;
    component::collision::set(sql, panel, true, false, false)?;
    Ok(panel)
}

pub fn init_wall(
    sql: &rusqlite::Connection,
    tile: &str,
    x: i64,
    y: i64,
) -> rusqlite::Result<entity::Entity> {
    let panel = entity::create(sql)?;
    component::actor::set(sql, panel, tile, x, y, Plane::Wall)?;
    component::collision::set(sql, panel, true, true, false)?;
    Ok(panel)
}

pub fn generate_particles(sql: &rusqlite::Connection, lifespan: i64) -> rusqlite::Result<()> {
    let entity = entity::create(sql)?;
    component::actor::set_on_random_empty_ground(sql, entity, "*", Plane::Particles)?;
    component::velocity::set_random(sql, entity, -1..=1)?;
    component::health::set(sql, entity, lifespan, lifespan, -1)?;
    component::collision::set(sql, entity, false, false, true)?;
    Ok(())
}

pub fn generate_enemies(sql: &rusqlite::Connection, lifespan: i64) -> rusqlite::Result<()> {
    let entity = entity::create(sql).unwrap();
    component::actor::set_on_random_empty_ground(sql, entity, "x", Plane::Enemies)?;
    component::velocity::set(sql, entity, 0, 0)?;
    component::health::set(sql, entity, lifespan, lifespan, -1)?;
    component::collision::set(sql, entity, false, true, false)?;
    component::ai::set_target_player(sql, entity)?;
    // component::ai::set_random(sql, entity)?;
    Ok(())
}
