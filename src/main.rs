mod component;
mod console;
mod entity;
mod game_object;
mod map_gen;
mod meta;
mod profiler;
mod system;

use crate::console::Console;
use console::ClickEvent;
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
        GgezState {
            console,
            state: State {
                rng,
                renderer: meta::Renderer::new(),
                mode: meta::GameMode::MainMenu(main_menu),
            },
        },
    );
}

struct GgezState {
    console: Console,
    state: State,
}

struct State {
    mode: meta::GameMode,
    renderer: meta::Renderer,
    rng: &'static Mutex<meta::GameRng>,
}

fn new_game<P: AsRef<Path>>(
    rng: &'static Mutex<meta::GameRng>,
    path: P,
    is_creative: bool,
    mut dungeon_generator: impl map_gen::Generator,
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
            let down_stairs = entity::create(&db)?;
            component::actor::set(
                &db,
                component::actor::Actor {
                    entity: down_stairs,
                    tile: ">".into(),
                    pos,
                    color: game_object::PLAYER_COLOR,
                    plane: game_object::Plane::Objects,
                },
            )?;
            component::transition::set(&db, down_stairs, game_object::WIN_LEVEL)?;
        } else if tile == Tile::UpStairs {
            game_object::init_floor(&db, pos)?;
            // Player spawns where the up staircase would be
            component::actor::set(
                &db,
                component::actor::Actor {
                    entity: player,
                    tile: "@".into(),
                    pos,
                    color: game_object::STAIR_COLOR,
                    plane: game_object::Plane::Player,
                },
            )?;
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

impl ggez::event::EventHandler<ggez::GameError> for GgezState {
    fn update(&mut self, ctx: &mut ggez::Context) -> GameResult {
        while ctx.time.check_update_time(DESIRED_FPS) {
            self.state
                .tick(&mut self.console, ctx)
                .expect("Unexpected error during game tick")
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut ggez::Context) -> GameResult {
        self.state
            .renderer
            .draw(&self.state.mode, &mut self.console, ctx)
            .expect("Unexpected error during game draw");
        Ok(())
    }
}

impl State {
    fn tick(&mut self, console: &mut Console, ctx: &mut ggez::Context) -> anyhow::Result<()> {
        // Game loop.
        let keys = console.key_presses(ctx);
        match self.mode {
            meta::GameMode::MainMenu(ref mut menu) => {
                let selected = meta::keydown_handler(&keys, menu);
                match selected {
                    meta::MenuResult::None => {}
                    meta::MenuResult::Updated => {
                        self.renderer.mark_dirty();
                    }
                    meta::MenuResult::Selected(meta::NEW_GAME) => {
                        self.mode = new_game(
                            self.rng,
                            meta::SAVE_FILE_NAME,
                            false,
                            map_gen::DefaultGenerator::new(),
                        )?;
                        self.renderer.mark_dirty();
                    }
                    meta::MenuResult::Selected(meta::LOAD_GAME) => {
                        self.mode = load_game(self.rng, meta::SAVE_FILE_NAME)?;
                        self.renderer.mark_dirty();
                    }
                    meta::MenuResult::Selected(meta::CREATIVE_MODE) => {
                        self.mode = new_game(
                            self.rng,
                            meta::SAVE_FILE_NAME,
                            true,
                            map_gen::EmptyGenerator,
                        )?;
                        self.renderer.mark_dirty();
                    }
                    meta::MenuResult::Selected(selected) => {
                        println!("Unexpected menu item '{}'. This is a bug", selected)
                    }
                    meta::MenuResult::Back => {
                        console.quit(ctx);
                    }
                }
            }
            meta::GameMode::InGame {
                ref db,
                player,
                mut profiler,
                is_creative: _is_creative,
                ref mut selected_point,
            } => {
                let clicks = console.clicks(ctx);
                if let Some(ClickEvent { pos, click_type: _ }) = clicks.into_iter().nth(0) {
                    *selected_point = Some(pos);
                    self.renderer.mark_dirty();
                }
                let new_mode = meta::in_game_keydown_handler(db, &keys, player)?;

                if let Some(meta::GameMode::WonGame) = new_mode {
                    self.mode = meta::GameMode::WonGame;
                    self.renderer.mark_dirty();
                } else if component::player::outstanding_turns(db)? > 0 {
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
                        game_object::generate_particles(db, 25)?;
                    }
                    turn.split("particles");
                    for _ in 0..5 {
                        game_object::generate_enemies(db, 10)?;
                    }
                    turn.split("enemies");
                    system::cull_dead(db)?;
                    system::cull_ephemeral(db)?;
                    turn.split("culling");
                    let turn_num = component::player::turns_passed(db)?;

                    let actor_count = component::actor::count(db)?;
                    db.execute_batch("COMMIT TRANSACTION")?;

                    profiler.end(db, turn_num, turn, actor_count)?;
                    self.renderer.mark_dirty();
                }
            }
            meta::GameMode::WonGame => {
                meta::won_game_keydown_handler(&keys, &mut self.mode);
                self.renderer.mark_dirty();
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
