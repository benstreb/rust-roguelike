mod component;
mod entity;
mod game_object;
mod map_gen;
mod menu;
mod meta;
mod profiler;
mod system;

use bracket_lib::prelude::{main_loop, BResult, BTerm, BTermBuilder, GameState, VirtualKeyCode};
use map_gen::{Generator, Tile};
use menu::main_menu;
use rand::{Rng, SeedableRng};
use std::{path::Path, sync::Mutex};

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

    // Placeholder for game engine init
    let mut console = BTermBuilder::simple80x50()
        .with_title("Hello Rust World")
        .build()?;

    let main_menu = menu::main_menu();
    main_menu.draw(&mut console);

    main_loop(
        console,
        State {
            turn_profiler,
            rng,
            mode: meta::GameMode::MainMenu(main_menu),
        },
    )
}

struct State {
    turn_profiler: profiler::TurnProfiler,
    mode: meta::GameMode,
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
}

fn new_game<P: AsRef<Path>>(
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
    path: P,
    console: &mut BTerm,
) -> BResult<meta::GameMode> {
    std::fs::remove_file(&path)?;
    let db = open_db(path, rng)?;

    db.execute_batch("BEGIN TRANSACTION")?;
    entity::create_table(&db)?;
    component::create_tables(&db)?;

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
            component::actor::set(
                &db,
                down_stairs,
                ">",
                x,
                y,
                game_object::PLAYER_COLOR.into(),
                game_object::Plane::Objects,
            )?;
            component::transition::set(&db, down_stairs, game_object::WIN_LEVEL)?;
        } else if tile == Tile::UpStairs {
            game_object::init_floor(&db, x, y)?;
            // Player spawns where the up staircase would be
            component::actor::set(
                &db,
                player,
                "@",
                x,
                y,
                game_object::STAIR_COLOR.into(),
                game_object::Plane::Player,
            )?;
        }
    }

    draw_screen(&db, console, 0)?;

    db.execute_batch("COMMIT TRANSACTION")?;

    Ok(meta::GameMode::InGame { db, player })
}

fn load_game<P: AsRef<Path>>(
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
    path: P,
    console: &mut BTerm,
) -> BResult<meta::GameMode> {
    let db = open_db(path, rng)?;
    let player = entity::load_player(&db)?;
    let turn = component::player::turns_passed(&db)?;
    draw_screen(&db, console, turn)?;
    Ok(meta::GameMode::InGame { db, player })
}

impl State {
    fn tick_inner(&mut self, console: &mut BTerm) -> BResult<()> {
        // Game loop.
        match self.mode {
            meta::GameMode::MainMenu(ref mut menu) => {
                menu.draw(console);

                let selected = menu::keydown_handler(console.key, menu);
                match selected {
                    menu::MenuResult::None => {}
                    menu::MenuResult::Selected(menu::NEW_GAME) => {
                        self.mode = new_game(self.rng, meta::SAVE_FILE_NAME, console)?;
                    }
                    menu::MenuResult::Selected(menu::LOAD_GAME) => {
                        self.mode = load_game(self.rng, meta::SAVE_FILE_NAME, console)?;
                    }
                    menu::MenuResult::Selected(selected) => {
                        println!(
                            "You selected {}. This is just for testing and doesn't do anything",
                            selected
                        )
                    }
                    menu::MenuResult::Back => {
                        console.quit();
                    }
                }
            }
            meta::GameMode::InGame { ref db, player } => {
                let new_mode = in_game_keydown_handler(db, console.key, player)?;

                if let Some(meta::GameMode::WonGame) = new_mode {
                    self.mode = meta::GameMode::WonGame;
                } else if component::player::outstanding_turns(db)? > 0 {
                    db.execute_batch("BEGIN TRANSACTION")?;
                    let turn_start = self.turn_profiler.start();
                    system::apply_ai(db)?;
                    system::move_actors(db)?;
                    component::player::pass_time(db, 1)?;
                    system::apply_regen(db)?;
                    for _ in 0..25 {
                        game_object::generate_particles(db, 25)?;
                    }
                    for _ in 0..5 {
                        game_object::generate_enemies(db, 10)?;
                    }
                    system::cull_dead(db)?;
                    system::cull_ephemeral(db)?;
                    let turn = component::player::turns_passed(db)?;

                    draw_screen(db, console, turn)?;

                    let actor_count = component::actor::count(db)?;
                    db.execute_batch("COMMIT TRANSACTION")?;

                    self.turn_profiler.end(turn, turn_start, actor_count)?;
                }
            }
            meta::GameMode::WonGame => {
                won_game_keydown_handler(console.key, &mut self.mode);

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

fn open_db<P: AsRef<Path>>(
    path: P,
    rng: &'static Mutex<rand_pcg::Pcg64Mcg>,
) -> rusqlite::Result<rusqlite::Connection> {
    let db = rusqlite::Connection::open(path)?;

    rusqlite::vtab::series::load_module(&db)?;
    add_pcg_randint_function(&db, rng)?;

    db.execute_batch("PRAGMA foreign_keys = TRUE")?;

    Ok(db)
}

fn draw_screen(db: &rusqlite::Connection, console: &mut BTerm, turn: i64) -> BResult<()> {
    console.cls();
    system::draw_actors(db, console)?;
    console.print(0, 0, turn.to_string());
    Ok(())
}

fn in_game_keydown_handler(
    db: &rusqlite::Connection,
    keycode: Option<VirtualKeyCode>,
    player: entity::Entity,
) -> rusqlite::Result<Option<meta::GameMode>> {
    if component::player::outstanding_turns(db)? > 0 {
        return Ok(None);
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
            if new_level == Some(game_object::WIN_LEVEL.to_string()) {
                return Ok(Some(meta::GameMode::WonGame));
            }
        }
        _ => {}
    };
    Ok(None)
}

fn won_game_keydown_handler(keycode: Option<VirtualKeyCode>, mode: &mut meta::GameMode) {
    if keycode.is_some() {
        *mode = meta::GameMode::MainMenu(main_menu())
    }
}
