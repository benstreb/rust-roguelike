use crate::entity;
use crate::game_object;

use rusqlite::params;

pub fn create_tables(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    player::create_table(db)?;
    actor::create_table(db)?;
    velocity::create_table(db)?;
    ai::create_table(db)?;
    collision::create_table(db)?;
    collision::create_passable_tiles_view(db)?;
    health::create_table(db)?;
    transition::create_table(db)?;
    Ok(())
}

pub mod player {
    use rusqlite::named_params;

    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS Player (
                entity INTEGER UNIQUE NOT NULL,
                turn INTEGER,
                outstanding_turns INTEGER,
                level TEXT,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn set(db: &rusqlite::Connection, entity: entity::Entity) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Player (entity, turn, outstanding_turns, level)
            VALUES (?, 0, 0, '0')",
            params![entity],
        )?;
        Ok(())
    }

    pub fn pass_time(db: &rusqlite::Connection, turns: i64) -> rusqlite::Result<()> {
        db.execute(
            "UPDATE Player
                SET turn = turn + max(outstanding_turns - :turns, 0),
                    outstanding_turns = max(outstanding_turns - :turns, 0)",
            named_params! {":turns": turns},
        )?;
        Ok(())
    }

    pub fn outstanding_turns(db: &rusqlite::Connection) -> rusqlite::Result<i64> {
        db.query_row("SELECT outstanding_turns FROM Player LIMIT 1", (), |row| {
            row.get(0)
        })
    }

    pub fn turns_passed(db: &rusqlite::Connection) -> rusqlite::Result<i64> {
        db.query_row("SELECT turn FROM Player LIMIT 1", (), |row| row.get(0))
    }

    pub fn level(db: &rusqlite::Connection) -> rusqlite::Result<String> {
        db.query_row("SELECT level FROM Player LIMIT 1", (), |row| row.get(0))
    }
}

pub mod actor {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Actor (
                entity INTEGER NOT NULL,
                tile TEXT,
                x INTEGER,
                y INTEGER,
                plane INTEGER,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_actor_entity ON Actor (entity ASC);
            CREATE INDEX IF NOT EXISTS idx_actor_plane ON Actor (plane DESC);
            CREATE INDEX IF NOT EXISTS idx_actor_position ON Actor (x ASC, y ASC);
        ",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        tile: &str,
        x: i64,
        y: i64,
        plane: game_object::Plane,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Actor (entity, tile, x, y, plane)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET tile = excluded.tile, x = excluded.x, y = excluded.y, plane = excluded.plane",
            params![entity, tile, x, y, plane],
        )?;
        Ok(())
    }

    pub fn set_on_random_empty_ground(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        tile: &str,
        plane: game_object::Plane,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Actor (entity, tile, x, y, plane)
            SELECT ?, ?, x, y, ?
            FROM Actor
            WHERE Actor.entity IN (SELECT entity FROM PassableTiles)
            ORDER BY RANDOM()
            LIMIT 1",
            params![entity, tile, plane],
        )?;
        Ok(())
    }

    pub fn count(db: &rusqlite::Connection) -> rusqlite::Result<i64> {
        db.query_row("SELECT COUNT(*) FROM Actor", (), |row| row.get(0))
    }
}

pub mod velocity {
    use std::ops::RangeInclusive;

    use rusqlite::named_params;

    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Velocity (
                entity INTEGER NOT NULL,
                dx INTEGER,
                dy INTEGER,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_velocity_entity ON Velocity (entity ASC);
        ",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        dx: i64,
        dy: i64,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Velocity (entity, dx, dy)
            VALUES (?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET dx = excluded.dx, dy = excluded.dy",
            params![entity, dx, dy],
        )?;
        Ok(())
    }

    pub fn set_random(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        range: RangeInclusive<i64>,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Velocity (entity, dx, dy)
            VALUES (:entity, pcg_randint(:min, :max), pcg_randint(:min, :max))
            ON CONFLICT (entity) DO UPDATE SET dx = excluded.dx, dy = excluded.dy",
            named_params![":entity": entity, ":min": range.start(), ":max": range.end()],
        )?;
        Ok(())
    }
}

pub mod collision {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Collision (
                entity INTEGER UNIQUE NOT NULL,
                ground BOOLEAN,
                solid BOOLEAN,
                ephemeral BOOLEAN,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn create_passable_tiles_view(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE VIEW IF NOT EXISTS PassableTiles AS
            SELECT Collision.entity, Actor.x, Actor.y
            FROM Collision
            JOIN Actor ON Actor.entity = Collision.entity
            WHERE Collision.ground = 1
            AND Collision.entity NOT IN (
                SELECT Collision.entity
                FROM Collision
                JOIN Actor ON Actor.entity = Collision.entity
                JOIN Actor ground_actor ON ground_actor.x = Actor.x AND ground_actor.y = Actor.y
                JOIN Collision ground_collision ON ground_collision.entity = ground_actor.entity
                WHERE Collision.solid = 1 AND ground_collision.ground = 1
            )
        ",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        ground: bool,
        solid: bool,
        ephemeral: bool,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Collision (entity, ground, solid, ephemeral)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET ground = excluded.ground, solid = excluded.solid, ephemeral = excluded.ephemeral",
            params![entity, ground, solid, ephemeral],
        )?;
        Ok(())
    }
}

pub mod health {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Health (
                entity INTEGER UNIQUE NOT NULL,
                max INTEGER,
                current INTEGER,
                regen INTEGER,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        max: i64,
        current: i64,
        regen: i64,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Health (entity, max, current, regen)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET max = excluded.max, current = excluded.current, regen = excluded.regen",
            params![entity, max, current, regen],
        )?;
        Ok(())
    }
}

pub mod ai {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Ai (
                entity INTEGER UNIQUE NOT NULL,
                random BOOLEAN,
                target_player BOOLEAN,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn set_random(db: &rusqlite::Connection, entity: entity::Entity) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Ai (entity, random, target_player)
            VALUES (?, TRUE, FALSE)
            ON CONFLICT (entity) DO UPDATE SET random = excluded.random, target_player = excluded.target_player",
            params![entity],
        )?;
        Ok(())
    }

    pub fn set_target_player(
        db: &rusqlite::Connection,
        entity: entity::Entity,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Ai (entity, random, target_player)
            VALUES (?, FALSE, TRUE)
            ON CONFLICT (entity) DO UPDATE SET random = excluded.random, target_player = excluded.target_player",
            params![entity],
        )?;
        Ok(())
    }
}

pub mod transition {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Transition (
                entity INTEGER UNIQUE NOT NULL,
                level TEXT,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        level: &str,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Transition (entity, level)
            VALUES (?, ?)
            ON CONFLICT (entity) DO UPDATE SET level = excluded.level",
            params![entity, level],
        )?;
        Ok(())
    }
}
