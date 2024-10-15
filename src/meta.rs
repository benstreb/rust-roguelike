use crate::console::{self, ConsolePoint, VirtualKeyCode};
use crate::profiler::TurnProfiler;
use crate::{component, entity, game_object, system};
use rand::SeedableRng;
use std::collections::HashSet;
use std::fmt::Debug;
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

impl GameMode {
    pub fn input_handler(
        &mut self,
        keycodes: &HashSet<VirtualKeyCode>,
        clicks: &HashSet<console::ClickEvent>,
    ) -> rusqlite::Result<GameEvent> {
        use Action::*;
        use GameEvent::*;

        match self {
            GameMode::MainMenu(menu) => Ok(menu.keydown_handler(keycodes)),
            GameMode::WonGame => {
                if keycodes.len() > 0 {
                    Ok(GameEvent::ReturnToMainMenu)
                } else {
                    Ok(GameEvent::None)
                }
            }
            GameMode::InGame { db, player, .. } => {
                if let Some(console::ClickEvent { pos, click_type: _ }) = clicks.into_iter().nth(0)
                {
                    return Ok(GameEvent::Click(*pos));
                }

                if component::player::outstanding_turns(db)? > 0 {
                    return Ok(None);
                }

                keycodes
                    .iter()
                    .filter_map(|keycode| match keycode {
                        VirtualKeyCode::Left => Some(Move(game_object::Direction::West)),
                        VirtualKeyCode::Right => Some(Move(game_object::Direction::East)),
                        VirtualKeyCode::Up => Some(Move(game_object::Direction::North)),
                        VirtualKeyCode::Down => Some(Move(game_object::Direction::South)),
                        VirtualKeyCode::Space | VirtualKeyCode::NumpadEnter => Some(Interact),
                        _ => Option::None,
                    })
                    .map(|event| match event {
                        Move(direction) => {
                            let (dx, dy) = direction.delta();
                            component::velocity::set(db, *player, dx, dy)?;
                            component::player::schedule_time(db, 1)?;
                            Ok(PassTime)
                        }
                        Interact => {
                            let new_level = system::follow_transition(db)?;
                            if new_level == Some(game_object::WIN_LEVEL.to_string()) {
                                return Ok(WinGame);
                            }
                            Ok(None)
                        }
                    })
                    .find(|event| match event {
                        Ok(GameEvent::None) => false,
                        _ => true,
                    })
                    .unwrap_or(Ok(GameEvent::None))
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum GameEvent {
    #[default]
    None,
    Refresh,
    Back,
    NewGame {
        is_creative: bool,
    },
    LoadGame,
    PassTime,
    WinGame,
    ReturnToMainMenu,
    Click(ConsolePoint),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Action {
    Move(game_object::Direction),
    Interact,
}

#[derive(Clone)]
pub struct Menu {
    pub top_left: console::ConsolePoint,
    pub selected: usize,
    pub items: Arc<Vec<String>>,
    selection_handler: Arc<dyn Fn(&str) -> GameEvent>,
}

impl Debug for Menu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Menu")
            .field("top_left", &self.top_left)
            .field("selected", &self.selected)
            .field("items", &self.items)
            .finish()
    }
}

impl Menu {
    pub fn keydown_handler(&mut self, keycodes: &HashSet<VirtualKeyCode>) -> GameEvent {
        for keycode in keycodes {
            match keycode {
                VirtualKeyCode::Left | VirtualKeyCode::Up => {
                    self.add(-1);
                    return GameEvent::Refresh;
                }
                VirtualKeyCode::Right | VirtualKeyCode::Down => {
                    self.add(1);
                    return GameEvent::Refresh;
                }
                VirtualKeyCode::Space | VirtualKeyCode::NumpadEnter | VirtualKeyCode::Return => {
                    return (self.selection_handler)(&self.items[self.selected]);
                }
                VirtualKeyCode::Escape => return GameEvent::Back,
                _ => {}
            }
        }
        GameEvent::None
    }
}

pub fn main_menu() -> Menu {
    static MAIN_MENU_ITEMS: LazyLock<Arc<Vec<String>>> = LazyLock::new(|| {
        Arc::new(vec![
            CREATIVE_MODE.to_string(),
            NEW_GAME.to_string(),
            LOAD_GAME.to_string(),
        ])
    });
    fn main_menu_handler(selection: &str) -> GameEvent {
        match selection {
            CREATIVE_MODE => GameEvent::NewGame { is_creative: true },
            NEW_GAME => GameEvent::NewGame { is_creative: false },
            LOAD_GAME => GameEvent::LoadGame,
            _ => unreachable!("Unexpected selection {:?} in main_menu_handler", selection),
        }
    }
    Menu {
        top_left: ConsolePoint { x: 0, y: 0 },
        selected: 0,
        items: MAIN_MENU_ITEMS.clone(),
        selection_handler: Arc::new(main_menu_handler),
    }
}

impl Menu {
    pub fn add(&mut self, i: i64) {
        self.selected = (self.selected as i64 + i).rem_euclid(self.items.len() as i64) as usize
    }
}
