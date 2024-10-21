use rusqlite::{named_params, OptionalExtension};

use crate::{component, entity, game_object};

pub fn move_actors(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
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

pub fn follow_transition(db: &rusqlite::Connection) -> rusqlite::Result<Option<String>> {
    db.query_row(
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
    .optional()
}

pub fn apply_ai(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute(
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
            AND Ai.type = :type",
        named_params! {":type": component::ai::AI_TYPE_RANDOM},
    )?;

    Ok(())
}

pub fn apply_regen(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch("UPDATE Health SET current = current + regen")?;
    Ok(())
}

pub fn cull_dead(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
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

pub fn cull_ephemeral(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
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

pub fn get_visible(db: &rusqlite::Connection) -> rusqlite::Result<Vec<game_object::Object>> {
    let mut query = db.prepare(
        "
        SELECT Actor.*, Tile.*, Temp.ground_degrees,  min(plane)
        FROM Actor
        JOIN Tile on Actor.entity = Tile.entity
        JOIN (
            SELECT x AS t_x, y AS t_y, degrees AS ground_degrees
            FROM FloatingTemp
            JOIN Actor ON FloatingTemp.entity = Actor.entity
            JOIN Collision ON Actor.entity = Collision.entity
            WHERE Collision.ground = 1
        ) AS Temp ON Actor.x = t_x AND Actor.y = t_y
        GROUP BY x, y",
    )?;
    let result = query
        .query_map((), |row| {
            let entity: entity::Entity = row.get("entity")?;
            let icon: String = row.get("icon")?;
            let x: i64 = row.get("x")?;
            let y: i64 = row.get("y")?;
            let r: u8 = row.get("r")?;
            let g: u8 = row.get("g")?;
            let b: u8 = row.get("b")?;
            let plane: game_object::Plane = row.get("plane")?;
            let ground_degrees: i64 = row.get("ground_degrees")?;
            let bg_color = if ground_degrees < 70 {
                game_object::Color {
                    r: 0,
                    g: 0,
                    b: ((70 - ground_degrees) * 10).min(255) as u8,
                }
            } else if ground_degrees > 100 {
                game_object::Color {
                    r: ((ground_degrees - 100) * 10).min(255) as u8,
                    g: 0,
                    b: 0,
                }
            } else {
                game_object::BACKGROUND_COLOR
            };
            Ok(game_object::Object {
                tile: component::tile::Tile {
                    entity,
                    icon,
                    color: game_object::Color { r, g, b },
                    plane,
                },
                pos: game_object::WorldPoint { x, y },
                bg_color,
            })
        })?
        .collect::<rusqlite::Result<Vec<game_object::Object>>>()?;
    Ok(result)
}

pub fn realize_spawns(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "-- Spawns with exact locations
        INSERT INTO Actor
        SELECT entity, x, y
        FROM SpawnAttempt
        WHERE spawn_rule = 0 -- SpawnRule::Exact
        ON CONFLICT (entity) DO UPDATE SET x = excluded.x, y = excluded.y;

        -- Random spawns
        INSERT INTO Actor (entity, x, y)
        SELECT entity, x, y
        FROM (
            SELECT
                entity,
                row_number() OVER (ORDER BY rowid) AS row_number
            FROM SpawnAttempt
            WHERE SpawnAttempt.spawn_rule = 1) AS Attempt
        JOIN (
            SELECT
                x,
                y,
                row_number() OVER (ORDER BY pcg_random()) AS row_number
            FROM PassableTiles
        ) AS Candidates ON Candidates.row_number = Attempt.row_number
        ON CONFLICT (entity) DO UPDATE SET x = excluded.x, y = excluded.y;

        -- Delete actors that didn't spawn
        -- TODO: implement when there's a meaningful chance of spawns failing.

        -- Spawn attempts only have one chance to resolve.
        DELETE FROM SpawnAttempt;
        ",
    )
}

pub fn update_temperature(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "-- Force floating temperatures to match fixed temperatures for the same entity
        UPDATE FloatingTemp
        SET degrees = FixedTemp.degrees
        FROM FixedTemp
        WHERE FixedTemp.entity = FloatingTemp.entity;

        -- Entities in the same space share body temperature
        UPDATE FloatingTemp
        SET degrees = average_degrees
        FROM (
            SELECT x, y, CAST(AVG(degrees) AS INTEGER) AS average_degrees
            FROM FloatingTemp
            JOIN Actor ON FloatingTemp.entity = Actor.entity
            GROUP BY x, y
        ) AS TileTemp
        JOIN Actor ON Actor.x = TileTemp.x AND Actor.y = TileTemp.y
        WHERE FloatingTemp.entity = Actor.entity;

        -- Ground tiles share temperatures evenly between each other
        -- This query requires that all exterior tiles have fixed temperatures,
        -- since they will get skipped.
        UPDATE FloatingTemp
        SET degrees = CAST((FloatingTemp.degrees +
            North.degrees +
            South.degrees +
            East.degrees
            + West.degrees) / 5 AS INTEGER)
        FROM Actor
        JOIN Collision ON Actor.entity = Collision.entity
        JOIN HeatMap AS North ON Actor.x = North.x AND Actor.y = North.y - 1
        JOIN HeatMap AS South ON Actor.x = South.x AND Actor.y = South.y + 1
        JOIN HeatMap AS East ON Actor.x = East.x + 1 AND Actor.y = East.y
        JOIN HeatMap AS West ON Actor.x = West.x - 1 AND Actor.y = West.y
        WHERE FloatingTemp.entity = Actor.entity AND Collision.ground = 1;
        ",
    )
}
