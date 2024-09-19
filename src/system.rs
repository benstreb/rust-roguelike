use crate::game_object::{CONSOLE_HEIGHT, CONSOLE_WIDTH};
use crate::{component, entity, game_object};

use bracket_lib::pathfinding::{Algorithm2D, BaseMap, DijkstraMap, Point};
use bracket_lib::prelude::SmallVec;
use bracket_lib::terminal::BTerm;
use rusqlite::named_params;

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

pub fn follow_transition(sql: &rusqlite::Connection) -> rusqlite::Result<String> {
    sql.query_row(
        "
        UPDATE Player
        SET level = Transition.level
        FROM Transition
        JOIN Actor transition_actor ON transition_actor.entity = Transition.entity
        JOIN Actor player_actor
            ON player_actor.x = transition_actor.x
            AND player_actor.y = transition_actor.y
        WHERE player_actor.entity = Player.entity
        RETURNING level
        ",
        [],
        |row| row.get::<usize, String>(0),
    )
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

struct RowMap(Vec<(bool, i64, i64)>);
impl RowMap {
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
        if self.0[dest].0 {
            return Some(dest);
        }
        None
    }
}

impl Algorithm2D for RowMap {
    fn dimensions(&self) -> Point {
        Point::new(game_object::CONSOLE_WIDTH, game_object::CONSOLE_HEIGHT)
    }
}

impl BaseMap for RowMap {
    fn is_opaque(&self, idx: usize) -> bool {
        self.0[idx].0
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

    // Query the database for ground tiles that are not solid
    let mut stmt = sql.prepare(
        "SELECT
             ifnull(PassableTiles.entity, 0) AS passable,
             grid.x,
             grid.y
         FROM (
             SELECT x, y
                FROM (SELECT value AS y FROM generate_series(0, :height - 1)),
                    (SELECT value AS x FROM generate_series(0, :width - 1))
         ) AS grid
         LEFT JOIN PassableTiles on grid.x = PassableTiles.x AND grid.y = PassableTiles.y
         ORDER BY grid.y, grid.x",
    )?;
    let map = RowMap(
        stmt.query_map(
            named_params! {":width": CONSOLE_WIDTH, ":height": CONSOLE_HEIGHT},
            |row| {
                let passable: bool = row.get("passable")?;
                let x: i64 = row.get("x")?;
                let y: i64 = row.get("y")?;
                Ok((passable, x, y))
            },
        )?
        .collect::<rusqlite::Result<Vec<(bool, i64, i64)>>>()?,
    );

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
