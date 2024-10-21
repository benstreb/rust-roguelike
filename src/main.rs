mod component;
mod console;
mod entity;
mod game_object;
mod map_gen;
mod meta;
mod profiler;
mod renderer;
mod system;

use crate::console::Console;
use component::spawn_attempt::SpawnAttempt;
use ggez::{conf::WindowMode, ContextBuilder, GameResult};
use map_gen::Tile;
use profiler::TurnProfiler;
use rand::Rng;
use std::{path::Path, sync::Mutex};

const DESIRED_FPS: u32 = 60;

fn add_pcg_randint_function(
    db: &rusqlite::Connection,
    rng: &'static Mutex<meta::GameRng>,
) -> rusqlite::Result<()> {
    use rusqlite::functions::FunctionFlags;
    db.create_scalar_function(
        "pcg_randint",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DIRECTONLY,
        move |ctx| {
            let min = ctx.get::<i32>(0)?;
            let max = ctx.get::<i32>(1)?;
            let spread = (max - min).abs();
            let start = min.min(max);
            let num = rng.lock().unwrap().gen_range(start..=start + spread);
            Ok(num)
        },
    )?;
    db.create_scalar_function(
        "pcg_random",
        0,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DIRECTONLY,
        move |_ctx| {
            let num = rng.lock().unwrap().gen::<i64>();
            Ok(num)
        },
    )
}

fn main() -> anyhow::Result<()> {
    let rng = Box::leak(Box::new(Mutex::new(meta::init_rng())));

    // Placeholder for game engine init
    let (mut ctx, event_loop) = ContextBuilder::new("rust_roguelike", "Yours Truly")
        .window_mode(WindowMode {
            width: (console::PIXEL_WIDTH) * meta::CONSOLE_WIDTH as f32,
            height: (console::PIXEL_HEIGHT) * meta::CONSOLE_HEIGHT as f32,
            ..Default::default()
        })
        .build()?;

    let main_menu = meta::main_menu();
    let console = Console::new(&mut ctx);
    ggez::event::run(
        ctx,
        event_loop,
        State {
            console,
            rng,
            renderer: renderer::Renderer::new(),
            mode: Box::new(meta::GameMode::MainMenu(main_menu)),
        },
    );
}

struct State {
    console: Console,
    mode: Box<meta::GameMode>,
    renderer: renderer::Renderer,
    rng: &'static Mutex<meta::GameRng>,
}

fn new_game<P: AsRef<Path>>(
    rng: &'static Mutex<meta::GameRng>,
    path: P,
    is_creative: bool,
    mut dungeon_generator: Box<dyn map_gen::Generator>,
) -> anyhow::Result<meta::GameMode> {
    std::fs::remove_file(&path)?;
    let db = open_db(path, rng)?;

    db.execute_batch("BEGIN TRANSACTION")?;
    entity::create_table(&db)?;
    component::create_tables(&db)?;

    let player = game_object::init_player(&db, is_creative)?;
    let initial_dungeon = dungeon_generator.generate(
        &mut rng.lock().unwrap(),
        meta::WORLD_WIDTH,
        meta::WORLD_HEIGHT,
    );

    for (tile, x, y) in initial_dungeon.iter() {
        let pos = game_object::WorldPoint { x, y };

        if tile == Tile::Unused {
            continue;
        } else if tile == Tile::Floor || tile == Tile::Corridor {
            game_object::init_floor(&db, pos)?;
        } else if tile == Tile::Wall {
            game_object::init_wall(&db, "#", pos)?;
        } else if tile == Tile::ClosedDoor || tile == Tile::OpenDoor {
            game_object::init_floor(&db, pos)?; // doors aren't supported at this time
        } else if tile == Tile::DownStairs {
            game_object::init_floor(&db, pos)?;
            game_object::init_downstairs(&db, pos)?;
        } else if tile == Tile::UpStairs {
            game_object::init_floor(&db, pos)?;
            // Player spawns where the up staircase would be
            component::actor::set(&db, player, pos)?;
        }
    }
    db.execute_batch("COMMIT TRANSACTION")?;

    let profiler = TurnProfiler::new(&db)?;
    Ok(meta::GameMode::InGame {
        db,
        player,
        profiler,
        is_creative,
        selected_point: None,
    })
}

fn load_game<P: AsRef<Path>>(
    rng: &'static Mutex<meta::GameRng>,
    path: P,
) -> anyhow::Result<meta::GameMode> {
    let db = open_db(path, rng)?;
    let player = entity::load_player(&db)?;
    let is_creative = component::player::is_creative(&db)?;
    let profiler = TurnProfiler::new(&db)?;
    Ok(meta::GameMode::InGame {
        db,
        player,
        profiler,
        is_creative,
        selected_point: None,
    })
}

impl ggez::event::EventHandler<ggez::GameError> for State {
    fn update(&mut self, ctx: &mut ggez::Context) -> GameResult {
        while ctx.time.check_update_time(DESIRED_FPS) {
            self.tick(ctx).expect("Unexpected error during game tick")
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut ggez::Context) -> GameResult {
        self.renderer
            .draw(&self.mode, &mut self.console, ctx)
            .expect("Unexpected error during game draw");
        Ok(())
    }
}

impl State {
    fn tick(&mut self, ctx: &mut ggez::Context) -> anyhow::Result<()> {
        use crate::meta::{GameEvent::*, GameMode::*};
        // Game loop.
        let clicks = self.console.clicks(ctx);
        let keys = self.console.key_presses(ctx);
        let event = self.mode.input_handler(&keys, &clicks)?;

        match (event, &mut *self.mode) {
            (None, _) => {}
            (Refresh, _) => {
                self.renderer.mark_dirty();
            }
            (NewGame { is_creative }, _) => {
                self.mode = Box::new(new_game(
                    self.rng,
                    meta::SAVE_FILE_NAME,
                    is_creative,
                    if is_creative {
                        Box::new(map_gen::EmptyGenerator)
                    } else {
                        Box::new(map_gen::DefaultGenerator::new())
                    },
                )?);
                self.renderer.mark_dirty();
            }
            (LoadGame, _) => {
                self.mode = Box::new(load_game(self.rng, meta::SAVE_FILE_NAME)?);
                self.renderer.mark_dirty();
            }
            (Back, MainMenu(_)) => {
                self.console.quit(ctx);
            }
            (WinGame, InGame { .. }) => {
                self.mode = Box::new(WonGame);
                self.renderer.mark_dirty();
            }
            (Click(meta::ClickTarget::Other(pos)), InGame { selected_point, .. }) => {
                *selected_point = Some(pos);
                self.renderer.mark_dirty();
            }
            (
                Click(meta::ClickTarget::World(pos)),
                InGame {
                    db,
                    is_creative: true,
                    selected_point,
                    ..
                },
            ) => {
                game_object::generate_particles(db, 25, SpawnAttempt::Exact(pos))?;
                system::realize_spawns(db)?;
                *selected_point = Some(pos.into());
                self.renderer.mark_dirty();
            }
            (
                Click(meta::ClickTarget::World(pos)),
                InGame {
                    is_creative: false,
                    selected_point,
                    ..
                },
            ) => {
                *selected_point = Some(pos.into());
                self.renderer.mark_dirty();
            }
            (
                PassTime,
                InGame {
                    ref db,
                    player: _,
                    mut profiler,
                    is_creative: _is_creative,
                    selected_point: _,
                },
            ) => {
                db.execute_batch("BEGIN TRANSACTION")?;
                let mut turn = profiler.start();
                system::apply_ai(db)?;
                turn.split("ai");
                system::move_actors(db)?;
                turn.split("movement");
                component::player::pass_time(db, 1)?;
                turn.split("time");
                system::apply_regen(db)?;
                turn.split("regen");
                for _ in 0..25 {
                    game_object::generate_particles(db, 25, SpawnAttempt::AnyUnoccupied)?;
                }
                for _ in 0..5 {
                    game_object::generate_enemies(db, 10)?;
                }
                system::realize_spawns(db)?;
                turn.split("spawns");
                system::cull_dead(db)?;
                system::cull_ephemeral(db)?;
                turn.split("culling");
                let turn_num = component::player::turns_passed(db)?;

                let actor_count = component::actor::count(db)?;
                db.execute_batch("COMMIT TRANSACTION")?;

                profiler.end(db, turn_num, turn, actor_count)?;
                self.renderer.mark_dirty();
            }
            (ReturnToMainMenu, _) => {
                *self.mode = MainMenu(meta::main_menu());
                self.renderer.mark_dirty();
            }
            (event, mode) => {
                println!("Unhandled event {:?} in mode {:?}", event, mode);
            }
        }
        anyhow::Result::Ok(())
    }
}

fn open_db<P: AsRef<Path>>(
    path: P,
    rng: &'static Mutex<meta::GameRng>,
) -> rusqlite::Result<rusqlite::Connection> {
    let db = rusqlite::Connection::open(path)?;

    rusqlite::vtab::series::load_module(&db)?;
    add_pcg_randint_function(&db, rng)?;

    db.execute_batch("PRAGMA foreign_keys = TRUE")?;

    Ok(db)
}
