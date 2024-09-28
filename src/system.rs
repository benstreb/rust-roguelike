use rusqlite::{named_params, OptionalExtension};

use crate::component;

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
