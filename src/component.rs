use crate::entity;
use crate::game_object;

use rusqlite::{named_params, params};

pub fn create_tables(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    player::create_table(db)?;
    actor::create_table(db)?;
    tile::create_table(db)?;
    velocity::create_table(db)?;
    ai::create_table(db)?;
    collision::create_table(db)?;
    collision::create_passable_tiles_view(db)?;
    health::create_table(db)?;
    transition::create_table(db)?;
    spawn_attempt::create_table(db)?;
    temperature::create_table(db)?;
    Ok(())
}

pub mod player {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS Player (
                entity INTEGER UNIQUE NOT NULL,
                is_creative BOOLEAN,
                turn INTEGER,
                outstanding_turns INTEGER,
                level TEXT,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        is_creative: bool,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Player (entity, turn, outstanding_turns, level, is_creative)
            VALUES (:entity, 0, 0, '0', :is_creative)",
            named_params! {":entity": entity, ":is_creative": is_creative},
        )?;
        Ok(())
    }

    pub fn pass_time(db: &rusqlite::Connection, turns: i64) -> rusqlite::Result<()> {
        db.execute(
            "UPDATE Player
                SET turn = turn + :turns,
                    outstanding_turns = max(outstanding_turns - :turns, 0)",
            named_params! {":turns": turns},
        )?;
        Ok(())
    }

    pub fn schedule_time(db: &rusqlite::Connection, turns: i64) -> rusqlite::Result<()> {
        db.execute(
            "UPDATE Player
                SET outstanding_turns =outstanding_turns + :turns",
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

    pub fn is_creative(db: &rusqlite::Connection) -> rusqlite::Result<bool> {
        db.query_row("SELECT is_creative FROM Player LIMIT 1", (), |row| {
            row.get(0)
        })
    }
}

pub mod actor {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Actor (
                entity INTEGER NOT NULL,
                x INTEGER,
                y INTEGER,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_actor_entity ON Actor (entity ASC);
            CREATE INDEX IF NOT EXISTS idx_actor_position ON Actor (x ASC, y ASC);
        ",
        )
    }

    pub fn set(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        pos: game_object::WorldPoint,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Actor (entity, x, y)
            VALUES (:entity, :x, :y)
            ON CONFLICT (entity) DO UPDATE SET x = excluded.x, y = excluded.y",
            named_params![
                ":entity": entity,
                ":x": pos.x,
                ":y": pos.y,
            ],
        )?;
        Ok(())
    }

    pub fn count(db: &rusqlite::Connection) -> rusqlite::Result<i64> {
        db.query_row("SELECT COUNT(*) FROM Actor", (), |row| row.get(0))
    }
}

pub mod tile {
    use super::*;

    #[derive(Debug)]
    pub struct Tile {
        pub entity: entity::Entity,
        pub icon: String,
        pub color: game_object::Color,
        pub plane: game_object::Plane,
    }

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Tile (
                entity INTEGER NOT NULL,
                icon TEXT,
                r INTEGER,
                g INTEGER,
                B INTEGER,
                plane INTEGER,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_tile_entity ON Tile (entity ASC);
            CREATE INDEX IF NOT EXISTS idx_tile_plane ON Tile (plane DESC);
        ",
        )
    }

    pub fn set(db: &rusqlite::Connection, tile: Tile) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO Tile (entity, icon, r, g, b, plane)
            VALUES (:entity, :icon, :r, :g, :b, :plane)
            ON CONFLICT (entity) DO
                UPDATE SET
                    icon = excluded.icon,
                    r = excluded.r,
                    g = excluded.g,
                    b = excluded.b,
                    plane = excluded.plane",
            named_params![
                ":entity": tile.entity,
                ":icon": tile.icon,
                ":r": tile.color.r,
                ":g": tile.color.g,
                ":b": tile.color.b,
                ":plane": tile.plane,
            ],
        )?;
        Ok(())
    }
}

pub mod velocity {
    use std::ops::RangeInclusive;

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
        db.prepare_cached(
            "INSERT INTO Velocity (entity, dx, dy)
            VALUES (?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET dx = excluded.dx, dy = excluded.dy",
        )?
        .execute(params![entity, dx, dy])?;
        Ok(())
    }

    pub fn set_random(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        range: RangeInclusive<i64>,
    ) -> rusqlite::Result<()> {
        db.prepare_cached(
            "INSERT INTO Velocity (entity, dx, dy)
            VALUES (:entity, pcg_randint(:min, :max), pcg_randint(:min, :max))
            ON CONFLICT (entity) DO UPDATE SET dx = excluded.dx, dy = excluded.dy",
        )?
        .execute(named_params![":entity": entity, ":min": range.start(), ":max": range.end()])?;
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
        db.prepare_cached(
            "INSERT INTO Collision (entity, ground, solid, ephemeral)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET ground = excluded.ground, solid = excluded.solid, ephemeral = excluded.ephemeral",
        )?.execute(
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
        db.prepare_cached(
            "INSERT INTO Health (entity, max, current, regen)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (entity) DO UPDATE SET max = excluded.max, current = excluded.current, regen = excluded.regen",
        )?.execute(
            params![entity, max, current, regen],
        )?;
        Ok(())
    }
}

pub mod ai {
    use super::*;

    pub const AI_TYPE_RANDOM: &str = "random";

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS Ai (
                entity INTEGER UNIQUE NOT NULL,
                type TEXT,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn set_random(db: &rusqlite::Connection, entity: entity::Entity) -> rusqlite::Result<()> {
        db.prepare_cached(
            "INSERT INTO Ai (entity, type)
            VALUES (:entity, :type)
            ON CONFLICT (entity) DO UPDATE SET type = excluded.type",
        )?
        .execute(named_params! {":entity": entity, ":type": AI_TYPE_RANDOM})?;
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

pub mod spawn_attempt {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    pub enum SpawnAttempt {
        Exact(game_object::WorldPoint),
        AnyUnoccupied,
    }

    #[derive(Clone, Copy, Debug, num_enum::TryFromPrimitive)]
    #[repr(i64)]
    enum SpawnRule {
        Exact = 0,
        AnyUnoccupied = 1,
    }

    impl rusqlite::types::ToSql for SpawnRule {
        fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
            Ok((*self as i64).into())
        }
    }

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS SpawnAttempt (
                entity INTEGER UNIQUE NOT NULL,
                spawn_rule INTEGER NOT NULL,
                x INTEGER,
                y INTEGER,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            )",
        )
    }

    pub fn create(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        attempt: SpawnAttempt,
    ) -> rusqlite::Result<()> {
        let (rule, x, y) = match attempt {
            SpawnAttempt::Exact(pos) => (SpawnRule::Exact, Some(pos.x), Some(pos.y)),
            SpawnAttempt::AnyUnoccupied => (SpawnRule::AnyUnoccupied, None, None),
        };

        db.execute(
            "INSERT INTO SpawnAttempt (entity, spawn_rule, x, y)
            VALUES (:entity, :spawn_rule, :x, :y)",
            named_params! {":entity": entity, ":spawn_rule": rule, ":x": x, ":y": y},
        )?;
        Ok(())
    }
}

pub mod temperature {
    use super::*;

    pub fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS FloatingTemp (
                entity INTEGER UNIQUE NOT NULL,
                degrees INTEGER NOT NULL,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );
            
            CREATE TABLE IF NOT EXISTS FixedTemp (
                entity INTEGER UNIQUE NOT NULL,
                degrees INTEGER NOT NULL,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS HeatSource (
                entity INTEGER UNIQUE NOT NULL,
                heat_gen INTEGER NOT NULL,
                FOREIGN KEY (entity) REFERENCES Entity (id) ON DELETE CASCADE
            );

            CREATE VIEW IF NOT EXISTS HeatMap AS
            SELECT Actor.x, Actor.y, FloatingTemp.degrees
            FROM FloatingTemp
            JOIN Actor ON Actor.entity = FloatingTemp.entity
            JOIN Collision ON FloatingTemp.entity = Collision.entity
            WHERE Collision.ground = 1
            GROUP BY x, y
            LIMIT 1;
            ",
        )
    }

    pub fn set_floating(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        degrees: i64,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO FloatingTemp (entity, degrees)
            VALUES (:entity, :degrees)
            ON CONFLICT (entity) DO UPDATE SET degrees = excluded.degrees",
            named_params! {":entity": entity, ":degrees": degrees},
        )?;
        Ok(())
    }
    pub fn set_fixed(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        degrees: i64,
    ) -> rusqlite::Result<()> {
        set_floating(db, entity, degrees)?;
        db.execute(
            "INSERT INTO FixedTemp (entity, degrees)
            VALUES (:entity, :degrees)
            ON CONFLICT (entity) DO UPDATE SET degrees = excluded.degrees",
            named_params! {":entity": entity, ":degrees": degrees},
        )?;
        Ok(())
    }

    pub fn set_heat_source(
        db: &rusqlite::Connection,
        entity: entity::Entity,
        heat_gen: i64,
    ) -> rusqlite::Result<()> {
        db.execute(
            "INSERT INTO HeatSource (entity, heat_gen)
            VALUES (:entity, :heat_gen)
            ON CONFLICT (entity) DO UPDATE SET heat_gen = excluded.heat_gen",
            named_params! {":entity": entity, ":heat_gen": heat_gen},
        )?;
        Ok(())
    }
}
