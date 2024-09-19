mod component;
mod entity;
mod game_object;
mod map_gen;
mod menu;
mod profiler;
mod system;

use bracket_lib::prelude::{main_loop, BResult, BTerm, BTermBuilder, GameState, VirtualKeyCode};
use map_gen::{Generator, Tile};
use menu::main_menu;
use rand::{Rng, SeedableRng};
use std::sync::Mutex;

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
    let turn_profiler = profiler::TurnProfiler::new("./turn_log.csv")?;

    let rng = Box::leak(Box::new(Mutex::new(rand_pcg::Pcg64Mcg::from_entropy())));

    let conn = rusqlite::Connection::open_in_memory()?;
    // let conn = rusqlite::Connection::open("game.db")?;

    add_pcg_randint_function(&conn, rng)?;

    // Placeholder for game engine init
    let mut console = BTermBuilder::simple80x50()
        .with_title("Hello Rust World")
        .build()?;

    let main_menu = menu::main_menu();
    main_menu.draw(&mut console);

    main_loop(
        console,
        State {
            conn,
            turn_profiler,
            rng,
            mode: GameMode::MainMenu(main_menu),
        },
    )
}

struct State {
    conn: rusqlite::Connection,
    turn_profiler: profiler::TurnProfiler,
    mode: GameMode,
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
}

enum GameMode {
    MainMenu(menu::Menu),
    InGame { player: entity::Entity },
    WonGame,
}

impl GameMode {
    fn new_game(
        db: &rusqlite::Connection,
        rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
    ) -> BResult<GameMode> {
        db.execute_batch(
            "
            PRAGMA foreign_keys = TRUE;
            BEGIN TRANSACTION;
        ",
        )?;
        rusqlite::vtab::series::load_module(&db)?;

        entity::create_table(&db)?;
        component::create_tables(&db)?;
        db.execute_batch("END TRANSACTION")?;

        db.execute("BEGIN TRANSACTION", rusqlite::params![])?;
        let player = game_object::init_player(&db)?;
        let mut dungeon_generator = map_gen::DefaultGenerator::new();
        // let mut dungeon_generator = map_gen::EmptyGenerator;
        let initial_dungeon = dungeon_generator.generate(
            &mut rng.lock().unwrap(),
            game_object::CONSOLE_WIDTH,
            game_object::CONSOLE_HEIGHT - 1,
        );

        for (tile, x, y) in initial_dungeon.iter() {
            let y = y + 1; // top row is reserved for diagnostics

            if tile == Tile::Unused {
                continue;
            } else if tile == Tile::Floor || tile == Tile::Corridor {
                game_object::init_floor(&db, x, y)?;
            } else if tile == Tile::Wall {
                game_object::init_wall(&db, "#", x, y)?;
            } else if tile == Tile::ClosedDoor || tile == Tile::OpenDoor {
                game_object::init_floor(&db, x, y)?; // doors aren't supported at this time
            } else if tile == Tile::DownStairs {
                game_object::init_floor(&db, x, y)?;
                let down_stairs = entity::create(&db)?;
                component::actor::set(&db, down_stairs, ">", x, y, game_object::Plane::Objects)?;
                component::transition::set(&db, down_stairs, game_object::WIN_LEVEL)?;
            } else if tile == Tile::UpStairs {
                game_object::init_floor(&db, x, y)?;
                // Player spawns where the up staircase would be
                component::actor::set(&db, player, "@", x, y, game_object::Plane::Player)?;
            }
        }

        db.execute_batch("COMMIT TRANSACTION")?;
        Ok(GameMode::InGame { player: player })
    }
}

impl State {
    fn tick_inner(&mut self, mut console: &mut BTerm) -> BResult<()> {
        // Game loop.
        match self.mode {
            GameMode::MainMenu(ref mut menu) => {
                menu.draw(console);

                let selected = menu::keydown_handler(console.key, menu);
                match selected {
                    None => {}
                    Some("New Game") => {
                        self.mode = GameMode::new_game(&self.conn, self.rng)?;

                        system::draw_actors(&self.conn, &mut console)?;
                    }
                    Some(selected) => {
                        println!(
                            "You selected {}. This is just for testing and doesn't do anything",
                            selected
                        )
                    }
                }
            }
            GameMode::InGame { player } => {
                in_game_keydown_handler(&self.conn, console.key, player, &mut self.mode)?;

                if component::player::outstanding_turns(&self.conn)? > 0 {
                    self.conn.execute_batch("BEGIN TRANSACTION")?;
                    let turn_start = self.turn_profiler.start();
                    system::apply_ai(&self.conn)?;
                    system::move_actors(&self.conn)?;
                    component::player::pass_time(&self.conn, 1)?;
                    system::apply_regen(&self.conn)?;
                    for _ in 0..25 {
                        game_object::generate_particles(&self.conn, 25)?;
                    }
                    for _ in 0..5 {
                        game_object::generate_enemies(&self.conn, 10)?;
                    }
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
            }
            GameMode::WonGame => {
                won_game_keydown_handler(&self.conn, console.key, &mut self.mode);

                console.cls();
                console.print(1, 1, "You Win");
            }
        }
        BResult::Ok(())
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        self.tick_inner(ctx).expect("Fatal error in game loop.");
    }
}

fn in_game_keydown_handler(
    db: &rusqlite::Connection,
    keycode: Option<VirtualKeyCode>,
    player: entity::Entity,
    mode: &mut GameMode,
) -> rusqlite::Result<()> {
    if component::player::outstanding_turns(db)? > 0 {
        return Ok(());
    }
    match keycode {
        Some(VirtualKeyCode::Left) => {
            component::velocity::set(db, player, -1, 0)?;
            component::player::pass_time(db, -1)?;
        }
        Some(VirtualKeyCode::Right) => {
            component::velocity::set(db, player, 1, 0)?;
            component::player::pass_time(db, -1)?;
        }
        Some(VirtualKeyCode::Up) => {
            component::velocity::set(db, player, 0, -1)?;
            component::player::pass_time(db, -1)?;
        }
        Some(VirtualKeyCode::Down) => {
            component::velocity::set(db, player, 0, 1)?;
            component::player::pass_time(db, -1)?;
        }
        Some(VirtualKeyCode::Space) | Some(VirtualKeyCode::NumpadEnter) => {
            let new_level = system::follow_transition(db)?;
            if new_level == game_object::WIN_LEVEL {
                *mode = GameMode::WonGame;
            }
        }
        _ => {}
    };
    Ok(())
}

fn won_game_keydown_handler(
    _: &rusqlite::Connection,
    keycode: Option<VirtualKeyCode>,
    mode: &mut GameMode,
) {
    if keycode.is_some() {
        *mode = GameMode::MainMenu(main_menu())
    }
}
