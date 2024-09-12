use crate::{component, entity, game_object};

use bracket_lib::terminal::{BTerm, VirtualKeyCode};
use rand::Rng;

pub fn keydown_handler(
    sql: &rusqlite::Connection,
    keycode: Option<VirtualKeyCode>,
    player: entity::Entity,
) -> rusqlite::Result<()> {
    if component::player::outstanding_turns(sql)? > 0 {
        return Ok(());
    }
    match keycode {
        Some(VirtualKeyCode::Left) => {
            component::velocity::set(sql, player, -1, 0)?;
            component::player::pass_time(sql, -1)?;
        }
        Some(VirtualKeyCode::Right) => {
            component::velocity::set(sql, player, 1, 0)?;
            component::player::pass_time(sql, -1)?;
        }
        Some(VirtualKeyCode::Up) => {
            component::velocity::set(sql, player, 0, -1)?;
            component::player::pass_time(sql, -1)?;
        }
        Some(VirtualKeyCode::Down) => {
            component::velocity::set(sql, player, 0, 1)?;
            component::player::pass_time(sql, -1)?;
        }
        Some(VirtualKeyCode::Space) | Some(VirtualKeyCode::NumpadEnter) => {} //sys::follow_transition(sql)?,
        _ => {}
    };
    Ok(())
}

pub fn move_actors(sql: &rusqlite::Connection) -> rusqlite::Result<()> {
    sql.execute_batch(
        "
        -- Move the actor according to its velocity
        UPDATE Actor
        SET x = Actor.x + Velocity.dx, y = Actor.y + Velocity.dy
        FROM Velocity
        WHERE Velocity.entity = Actor.entity
        -- as long as it is not an actor that would move to a tile with solid colision
        AND Actor.entity NOT IN (
            SELECT Collision.entity
            FROM Collision
            JOIN Actor ON Actor.entity = Collision.entity
            JOIN Velocity ON Actor.entity = Velocity.entity
            LEFT JOIN Actor solid_actor ON solid_actor.x = Actor.x + Velocity.dx AND solid_actor.y = Actor.y + Velocity.dy
            JOIN Collision solid_collision ON solid_collision.entity = solid_actor.entity
            WHERE Collision.solid AND solid_collision.solid
        )
        ")
}

pub fn draw_actors(
    conn: &rusqlite::Connection,
    console: &mut BTerm,
) -> Result<(), rusqlite::Error> {
    let mut conn = conn.prepare("SELECT tile, x, y FROM Actor ORDER BY plane DESC")?;
    for row in conn.query_map((), |row| {
        let x: i64 = row.get("x")?;
        let y: i64 = row.get("y")?;
        let tile: String = row.get("tile")?;
        Ok((x, y, tile))
    })? {
        let (x, y, tile) = row?;
        console.print(x, y, tile);
    }
    Ok(())
}

pub fn generate_particles(
    sql: &rusqlite::Connection,
    rng: &mut rand_pcg::Pcg64Mcg,
    lifespan: i64,
) -> rusqlite::Result<()> {
    let entity = entity::create(sql)?;
    component::actor::set(
        sql,
        entity,
        "*",
        rng.gen_range(0..80),
        rng.gen_range(0..25),
        game_object::Plane::Particles,
    )?;
    component::velocity::set(sql, entity, rng.gen_range(-1..2), rng.gen_range(-1..2))?;
    // component::health::set(sql, entity, lifespan, lifespan, -1)?;
    component::collision::set(sql, entity, false, false, true)?;
    Ok(())
}
