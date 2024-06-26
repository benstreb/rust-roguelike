mod component;
mod entity;
mod game_object;
mod map_gen;
mod system;

use anyhow::Ok;
use bracket_lib::prelude::{main_loop, BResult, BTerm, BTermBuilder, GameState};
use map_gen::Tile;

fn main() -> BResult<()> {
    // let mut turn_log_file = std::fs::File::create("./turn_log.csv")?;

    // let tee = boost::iostreams::tee(std::io::stdout(), &mut turn_log_file);
    // let mut turn_log = boost::iostreams::stream(tee);
    // turn_log.write_all(b"turn,time (ms),actors\n")?;

    let conn = rusqlite::Connection::open_in_memory()?;
    // let conn = rusqlite::Connection::open("game.db")?;

    conn.execute_batch(
        "
        PRAGMA foreign_keys = TRUE;
        BEGIN TRANSACTION;
    ",
    )?;
    entity::create_table(&conn)?;
    component::create_tables(&conn)?;
    conn.execute_batch("END TRANSACTION")?;

    conn.execute("BEGIN TRANSACTION", rusqlite::params![])?;
    let player = game_object::init_player(&conn)?;
    let initial_dungeon = map_gen::Dungeon::generate_empty(
        game_object::CONSOLE_WIDTH,
        game_object::CONSOLE_HEIGHT - 1,
    );

    for (tile, x, y) in initial_dungeon.iter() {
        let y = y + 1; // top row is reserved for diagnostics

        if tile == Tile::Unused {
            continue;
        } else if tile == Tile::Floor || tile == Tile::Corridor {
            game_object::init_floor(&conn, x, y)?;
        } else if tile == Tile::Wall {
            game_object::init_wall(&conn, "#", x, y)?;
        } else if tile == Tile::ClosedDoor || tile == Tile::OpenDoor {
            game_object::init_floor(&conn, x, y)?; // doors aren't supported at this time
        } else if tile == Tile::DownStairs {
            game_object::init_floor(&conn, x, y)?;
            let down_stairs = entity::create(&conn)?;
            component::actor::set(&conn, down_stairs, ">", x, y, game_object::Plane::Objects)?;
            // component::transition::set(&sql, down_stairs, game_object::WIN_LEVEL);
        } else if tile == Tile::UpStairs {
            game_object::init_floor(&conn, x, y)?;
            // Player spawns where the up staircase would be
            component::actor::set(&conn, player, "@", x, y, game_object::Plane::Player)?;
        }
    }
    conn.execute_batch("COMMIT TRANSACTION")?;

    // Placeholder for game engine init
    let mut console = BTermBuilder::simple80x50()
        .with_title("Hello Rust World")
        .build()?;
    system::draw_actors(&conn, &mut console)?;

    main_loop(console, State { conn })
}

struct State {
    conn: rusqlite::Connection,
}

impl State {
    fn tick_inner(&mut self, mut console: &mut BTerm) -> BResult<()> {
        // Game loop.
        if component::player::level(&self.conn)? == game_object::WIN_LEVEL {
            console.cls();
            console.print(1, 1, "You Win");
        } else if component::player::outstanding_turns(&self.conn)? > 0 {
            self.conn.execute_batch("BEGIN TRANSACTION")?;
            // let turn_start = std::time::Instant::now();
            // system::apply_ai(&sql);
            // system::move_actors(&sql);
            component::player::pass_time(&self.conn, 1)?;
            // system::apply_regen(&sql);
            // for _ in 0..25 {
            //     system::generate_particles(&sql, &rng, 25);
            // }
            // for _ in 0..5 {
            //     system::generate_enemies(&sql, &rng, 10);
            // }
            // system::cull_dead(&sql);
            // system::cull_ephemeral(&sql);
            let turn = component::player::turns_passed(&self.conn)?;
            // let turn_end = std::time::Instant::now();
            // let turn_duration = turn_end.duration_since(turn_start);
            // let most_recent_turn_ms = turn_duration.as_millis() as i32;
            console.cls();
            system::draw_actors(&self.conn, &mut console)?;
            self.conn.execute_batch("COMMIT TRANSACTION")?;
            // turn_log.write_all(
            //     format!(
            //         "{},{},{}\n",
            //         turn,
            //         most_recent_turn_ms,
            //         component::actor::count(&sql)
            //     )
            //     .as_bytes(),
            // )?;
            console.print(0, 0, turn.to_string());
        }
        BResult::Ok(())
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        self.tick_inner(ctx).expect("Fatal error in game loop.");
    }
}
