mod component;
mod entity;
mod game_object;
mod map_gen;
mod system;

use bracket_lib::prelude::{main_loop, BResult, BTerm, BTermBuilder, GameState};
use map_gen::{Generator, Tile};
use rand::{Rng, SeedableRng};
use std::{io::Write, sync::Mutex};

fn add_pcg_randint_function(
    db: &rusqlite::Connection,
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
) -> rusqlite::Result<()> {
    use rusqlite::functions::FunctionFlags;
    const SQLITE_RANDINT_ARGC: i32 = 2;
    db.create_scalar_function(
        "pcg_randint",
        SQLITE_RANDINT_ARGC,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DIRECTONLY,
        move |ctx| {
            let min = ctx.get::<i32>(0)?;
            let max = ctx.get::<i32>(1)?;
            let spread = (max - min).abs();
            let start = min.min(max);
            let num = rng.lock().unwrap().gen_range(start..=start + spread);
            Ok(num)
        },
    )
}

fn main() -> BResult<()> {
    let turn_log_file = std::fs::File::create("./turn_log.csv")?;

    let rng = Box::leak(Box::new(Mutex::new(rand_pcg::Pcg64Mcg::from_entropy())));

    let conn = rusqlite::Connection::open_in_memory()?;
    // let conn = rusqlite::Connection::open("game.db")?;

    add_pcg_randint_function(&conn, rng)?;

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
    let initial_dungeon = map_gen::DefaultGenerator::new().generate(
        &mut rng.lock().unwrap(),
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

    let turn_profiler = TurnProfiler::new(turn_log_file)?;

    main_loop(
        console,
        State {
            player,
            conn,
            turn_profiler,
            rng,
        },
    )
}

struct State {
    player: entity::Entity,
    conn: rusqlite::Connection,
    turn_profiler: TurnProfiler,
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
}

impl State {
    fn tick_inner(&mut self, mut console: &mut BTerm) -> BResult<()> {
        // Game loop.
        system::keydown_handler(&self.conn, console.key, self.player)?;

        if component::player::level(&self.conn)? == game_object::WIN_LEVEL {
            console.cls();
            console.print(1, 1, "You Win");
        } else if component::player::outstanding_turns(&self.conn)? > 0 {
            self.conn.execute_batch("BEGIN TRANSACTION")?;
            let turn_start = self.turn_profiler.start();
            // system::apply_ai(&sql);
            system::move_actors(&self.conn)?;
            component::player::pass_time(&self.conn, 1)?;
            system::apply_regen(&self.conn)?;
            for _ in 0..25 {
                system::generate_particles(&self.conn, &mut self.rng.lock().unwrap(), 25)?;
            }
            // for _ in 0..5 {
            //     system::generate_enemies(&sql, &rng, 10);
            // }
            system::cull_dead(&self.conn)?;
            system::cull_ephemeral(&self.conn)?;
            console.cls();
            system::draw_actors(&self.conn, &mut console)?;

            let turn = component::player::turns_passed(&self.conn)?;
            let actor_count = component::actor::count(&self.conn)?;
            self.conn.execute_batch("COMMIT TRANSACTION")?;

            self.turn_profiler.end(turn, turn_start, actor_count)?;
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

struct TurnProfiler {
    file: std::fs::File,
}

impl TurnProfiler {
    fn new(mut file: std::fs::File) -> std::io::Result<Self> {
        writeln!(file, "turn,time (ms),actors")?;
        std::io::Result::Ok(TurnProfiler { file })
    }

    fn start(&mut self) -> TurnStart {
        TurnStart {
            start: std::time::Instant::now(),
        }
    }

    fn end(&mut self, turn: i64, start: TurnStart, actor_count: i64) -> std::io::Result<()> {
        let end = std::time::Instant::now();
        let duration = end.duration_since(start.start);
        let ms = duration.as_millis();
        writeln!(self.file, "{},{},{}", turn, ms, actor_count)
    }
}

struct TurnStart {
    start: std::time::Instant,
}
