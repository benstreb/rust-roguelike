use crate::{component, entity, game_object};

use bracket_lib::pathfinding::{Algorithm2D, BaseMap, DijkstraMap, Point};
use bracket_lib::prelude::SmallVec;
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

struct SimpleMap(Vec<bool>);
impl SimpleMap {
    fn valid_exit(&self, location: Point, offset: Point) -> Option<usize> {
        let destination = location + offset;
        if destination.x < 0
            || destination.x as i64 >= game_object::CONSOLE_WIDTH
            || destination.y < 0
            || destination.y as i64 >= game_object::CONSOLE_HEIGHT
        {
            return None;
        }
        let dest = self.point2d_to_index(destination);
        if self.0[dest] {
            return Some(dest);
        }
        None
    }
}

impl Algorithm2D for SimpleMap {
    fn dimensions(&self) -> Point {
        Point::new(game_object::CONSOLE_WIDTH, game_object::CONSOLE_HEIGHT)
    }
}

impl BaseMap for SimpleMap {
    fn is_opaque(&self, idx: usize) -> bool {
        self.0[idx]
    }

    fn get_available_exits(&self, idx: usize) -> SmallVec<[(usize, f32); 10]> {
        let mut exits = SmallVec::new();
        let location = self.index_to_point2d(idx);

        if let Some(idx) = self.valid_exit(location, Point::new(-1, 0)) {
            exits.push((idx, 1.0))
        }
        if let Some(idx) = self.valid_exit(location, Point::new(1, 0)) {
            exits.push((idx, 1.0))
        }
        if let Some(idx) = self.valid_exit(location, Point::new(0, -1)) {
            exits.push((idx, 1.0))
        }
        if let Some(idx) = self.valid_exit(location, Point::new(0, 1)) {
            exits.push((idx, 1.0))
        }

        exits
    }
}

pub fn apply_ai(sql: &rusqlite::Connection) -> rusqlite::Result<()> {
    sql.execute_batch(
        "
        -- This query randomly updates all velocities with a random AI Type to
        -- one of the 8 cardinal directions. The spurious-seeming cor.val field
        -- forces the RANDINT call in a correlated subquery, which forces it to
        -- be executed once per row. Otherwise every particle moves in the same
        -- direction
        UPDATE Velocity
        SET (dx, dy) = (
            SELECT
                CASE
                    WHEN r < 3 THEN -1
                    WHEN r > 4 THEN 1
                    ELSE 0
                END AS new_dx,
                CASE
                    WHEN r IN (1, 4, 7) THEN -1
                    WHEN r IN (0, 3, 6) THEN 1
                    ELSE 0
                END AS new_dy
            FROM (SELECT pcg_randint(0, 8) AS r WHERE cor.val = 1)
        )
        FROM Ai, (SELECT 1 AS val) AS cor
        WHERE Ai.entity = Velocity.entity
            AND Ai.random = TRUE",
    )?;

    // Create a map with the dimensions of the console
    let mut map = SimpleMap(vec![
        false;
        (game_object::CONSOLE_WIDTH * game_object::CONSOLE_HEIGHT)
            as usize
    ]);

    // Query the database for ground tiles that are not solid
    let mut stmt = sql.prepare(
        "SELECT x, y
         FROM Actor
         JOIN Collision ON Actor.entity = Collision.entity
         WHERE Collision.ground = TRUE AND NOT Collision.solid
         GROUP BY x, y",
    )?;
    let tiles = stmt.query_map((), |row| {
        let x: i64 = row.get(0)?;
        let y: i64 = row.get(1)?;
        Ok((x, y))
    })?;

    // Set the properties of the map based on the query results
    for tile in tiles {
        let (x, y) = tile?;
        let idx = map.point2d_to_index(Point::new(x, y));
        map.0[idx] = true;
    }

    // Query the database for the player's position
    let mut stmt = sql.prepare(
        "SELECT x, y
         FROM Actor
         JOIN Player ON Actor.entity = Player.entity
         LIMIT 1",
    )?;
    let player_pos = stmt.query_row((), |row| {
        let x: i32 = row.get(0)?;
        let y: i32 = row.get(1)?;
        Ok(Point::new(x, y))
    })?;

    // Query the database for actors with AI targeting the player
    let mut stmt = sql.prepare(
        "SELECT Actor.entity, x, y
         FROM Actor
         JOIN Ai ON Actor.entity = Ai.entity
         WHERE Ai.target_player = TRUE",
    )?;
    let actors = stmt.query_map((), |row| {
        let entity: entity::Entity = row.get(0)?;
        let x: i32 = row.get(1)?;
        let y: i32 = row.get(2)?;
        Ok((entity, Point::new(x, y)))
    })?;

    // Create a Dijkstra map for pathfinding
    let dijkstra_map = DijkstraMap::new(
        game_object::CONSOLE_WIDTH as usize,
        game_object::CONSOLE_HEIGHT as usize,
        &[map.point2d_to_index(player_pos)],
        &map,
        100.0,
    );

    // Update the velocity of each actor based on the Dijkstra map
    for actor in actors {
        let (entity, pos) = actor?;
        if let Some(path) =
            DijkstraMap::find_lowest_exit(&dijkstra_map, map.point2d_to_index(pos), &map)
        {
            let next_pos = map.index_to_point2d(path);
            let dx = next_pos.x - pos.x;
            let dy = next_pos.y - pos.y;
            component::velocity::set(sql, entity, dx as i64, dy as i64)?;
        }
    }

    Ok(())
}

pub fn apply_regen(sql: &rusqlite::Connection) -> rusqlite::Result<()> {
    sql.execute_batch("UPDATE Health SET current = current + regen")?;
    Ok(())
}

pub fn cull_dead(sql: &rusqlite::Connection) -> rusqlite::Result<()> {
    sql.execute_batch(
        "DELETE FROM Entity
        WHERE id IN (
            SELECT id
            FROM Entity
            JOIN Health ON Entity.id = Health.entity
            WHERE Health.current <= 0
        )",
    )?;
    Ok(())
}

pub fn cull_ephemeral(sql: &rusqlite::Connection) -> rusqlite::Result<()> {
    sql.execute_batch(
        "DELETE FROM Entity
        WHERE id IN (
            SELECT Collision.entity
            FROM Collision
            JOIN Actor ON Actor.entity = Collision.entity
            JOIN Actor solid_actor ON solid_actor.x = Actor.x AND solid_actor.y = Actor.y
            JOIN Collision solid_collision ON solid_collision.entity = solid_actor.entity
            WHERE Collision.ephemeral AND (solid_collision.solid OR Collision.entity NOT IN (
                SELECT Collision.entity
                FROM Collision
                JOIN Actor ON Actor.entity = Collision.entity
                JOIN Actor ground_actor ON ground_actor.x = Actor.x AND ground_actor.y = Actor.y
                JOIN Collision ground_collision ON ground_collision.entity = ground_actor.entity
                WHERE Collision.ephemeral AND ground_collision.ground
            ))
        )",
    )?;
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
    component::health::set(sql, entity, lifespan, lifespan, -1)?;
    component::collision::set(sql, entity, false, false, true)?;
    Ok(())
}

pub fn generate_enemies(sql: &rusqlite::Connection, lifespan: i64) -> rusqlite::Result<()> {
    let entity = entity::create(sql).unwrap();
    component::actor::set_on_random_empty_ground(sql, entity, "x", game_object::Plane::Enemies)?;
    component::velocity::set(sql, entity, 0, 0)?;
    component::health::set(sql, entity, lifespan, lifespan, -1)?;
    component::collision::set(sql, entity, false, true, false)?;
    component::ai::set_target_player(sql, entity)?;
    // component::ai::set_random(sql, entity)?;
    Ok(())
}
