use crate::console::{self, Console, ConsolePoint, VirtualKeyCode};
use crate::profiler::TurnProfiler;
use crate::{component, entity, game_object, system};
use rand::SeedableRng;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

pub type GameRng = rand_pcg::Pcg64Mcg;

pub fn init_rng() -> GameRng {
    GameRng::from_entropy()
}

pub const SAVE_FILE_NAME: &'static str = "game.db";

pub const NEW_GAME: &str = "New Game";
pub const LOAD_GAME: &str = "Load Game";
pub const CREATIVE_MODE: &str = "Creative Mode";

pub const CONSOLE_WIDTH: i64 = 80;
pub const CONSOLE_HEIGHT: i64 = 30;

pub const WORLD_TOP_LEFT: ConsolePoint = ConsolePoint { x: 0, y: 1 };
pub const WORLD_WIDTH: i64 = 80;
pub const WORLD_HEIGHT: i64 = 25;

#[derive(Debug)]
pub enum GameMode {
    MainMenu(Menu),
    InGame {
        db: rusqlite::Connection,
        player: entity::Entity,
        profiler: TurnProfiler,
        is_creative: bool,
        selected_point: Option<ConsolePoint>,
    },
    WonGame,
}

pub fn in_game_keydown_handler(
    db: &rusqlite::Connection,
    keycodes: &HashSet<VirtualKeyCode>,
    player: entity::Entity,
) -> rusqlite::Result<Option<GameMode>> {
    if component::player::outstanding_turns(db)? > 0 {
        return Ok(None);
    }
    for keycode in keycodes {
        match keycode {
            VirtualKeyCode::Left => {
                component::velocity::set(db, player, -1, 0)?;
                component::player::schedule_time(db, 1)?;
            }
            VirtualKeyCode::Right => {
                component::velocity::set(db, player, 1, 0)?;
                component::player::schedule_time(db, 1)?;
            }
            VirtualKeyCode::Up => {
                component::velocity::set(db, player, 0, -1)?;
                component::player::schedule_time(db, 1)?;
            }
            VirtualKeyCode::Down => {
                component::velocity::set(db, player, 0, 1)?;
                component::player::schedule_time(db, 1)?;
            }
            VirtualKeyCode::Space | VirtualKeyCode::NumpadEnter => {
                let new_level = system::follow_transition(db)?;
                if new_level == Some(game_object::WIN_LEVEL.to_string()) {
                    return Ok(Some(GameMode::WonGame));
                }
            }
            _ => {}
        };
    }
    Ok(None)
}

pub fn won_game_keydown_handler(keycode: &HashSet<VirtualKeyCode>, mode: &mut GameMode) {
    if keycode.len() > 0 {
        *mode = GameMode::MainMenu(main_menu())
    }
}

#[derive(Debug, Default)]
pub struct Renderer {
    dirty: bool,
}

impl Renderer {
    pub fn new() -> Self {
        Renderer { dirty: true }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn draw(
        &mut self,
        gamemode: &GameMode,
        console: &mut Console,
        ctx: &mut ggez::Context,
    ) -> rusqlite::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        console.cls(ctx);
        match gamemode {
            GameMode::MainMenu(menu) => Self::draw_menu(menu, console),
            GameMode::InGame {
                db, selected_point, ..
            } => {
                let visible_actors = component::actor::get_visible(db)?;
                Self::draw_actors(&visible_actors, console);
                let turn = component::player::turns_passed(db)?;
                console.print(ConsolePoint { x: 0, y: 0 }, &turn.to_string());
                if let Some(pos) = selected_point {
                    console.print(
                        ConsolePoint {
                            x: 0,
                            y: WORLD_HEIGHT + 2,
                        },
                        &format!("({:<2}, {:<2})", pos.x, pos.y),
                    );
                } else {
                    console.print(
                        ConsolePoint {
                            x: 0,
                            y: WORLD_HEIGHT + 2,
                        },
                        "Click something!",
                    );
                }
            }
            GameMode::WonGame => {
                console.cls(ctx);
                console.print(ConsolePoint { x: 1, y: 1 }, "You Win");
            }
        }
        console.finish(ctx).expect("I'm dead!");
        self.dirty = false;
        Ok(())
    }

    fn draw_actors(actors: &Vec<component::actor::Actor>, console: &mut Console) {
        for actor in actors {
            console.print_color(
                actor.pos.into(),
                actor.color,
                game_object::BACKGROUND_COLOR,
                &actor.tile,
            );
        }
    }

    fn draw_menu(menu: &Menu, console: &mut Console) {
        for (i, item) in menu.items.iter().enumerate() {
            let color: game_object::MenuColor;
            if i == menu.selected {
                color = game_object::MENU_COLOR_SELECTED;
            } else {
                color = game_object::MENU_COLOR_UNSELECTED;
            }
            console.print_color(menu.top_left.down(i as i64), color.fg, color.bg, item)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Menu {
    top_left: console::ConsolePoint,
    selected: usize,
    items: Arc<Vec<String>>,
}

pub enum MenuResult<'a> {
    None,
    Updated,
    Selected(&'a str),
    Back,
}

pub fn keydown_handler<'a>(
    keycodes: &HashSet<VirtualKeyCode>,
    menu: &'a mut Menu,
) -> MenuResult<'a> {
    for keycode in keycodes {
        match keycode {
            VirtualKeyCode::Left | VirtualKeyCode::Up => {
                menu.add(-1);
                return MenuResult::Updated;
            }
            VirtualKeyCode::Right | VirtualKeyCode::Down => {
                menu.add(1);
                return MenuResult::Updated;
            }
            VirtualKeyCode::Space | VirtualKeyCode::NumpadEnter | VirtualKeyCode::Return => {
                return MenuResult::Selected(&menu.items[menu.selected]);
            }
            VirtualKeyCode::Escape => return MenuResult::Back,
            _ => {}
        }
    }
    MenuResult::None
}

pub fn main_menu() -> Menu {
    static MAIN_MENU_ITEMS: LazyLock<Arc<Vec<String>>> = LazyLock::new(|| {
        Arc::new(vec![
            CREATIVE_MODE.to_string(),
            NEW_GAME.to_string(),
            LOAD_GAME.to_string(),
        ])
    });
    Menu {
        top_left: ConsolePoint { x: 0, y: 0 },
        selected: 0,
        items: MAIN_MENU_ITEMS.clone(),
    }
}

impl Menu {
    pub fn add(&mut self, i: i64) {
        self.selected = (self.selected as i64 + i).rem_euclid(self.items.len() as i64) as usize
    }
}
