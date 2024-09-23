use std::sync::{Arc, LazyLock};

use crate::{component, entity, game_object};

use bracket_lib::terminal::{BTerm, VirtualKeyCode};

pub const SAVE_FILE_NAME: &'static str = "game.db";

pub const NEW_GAME: &str = "New Game";
pub const LOAD_GAME: &str = "Load Game";

#[derive(Debug)]
pub enum GameMode {
    MainMenu(Menu),
    InGame {
        db: rusqlite::Connection,
        player: entity::Entity,
    },
    WonGame,
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

    pub fn draw(&mut self, gamemode: &GameMode, console: &mut BTerm) -> rusqlite::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        console.cls();
        match gamemode {
            GameMode::MainMenu(menu) => Self::draw_menu(menu, console),
            GameMode::InGame { db, .. } => {
                let visible_actors = component::actor::get_visible(db)?;
                Self::draw_actors(&visible_actors, console);
                let turn = component::player::turns_passed(db)?;
                console.print(0, 0, turn.to_string());
            }
            GameMode::WonGame => {
                console.cls();
                console.print(1, 1, "You Win");
            }
        }
        self.dirty = false;
        Ok(())
    }

    fn draw_actors(actors: &Vec<component::actor::Actor>, console: &mut BTerm) {
        for actor in actors {
            console.print_color(
                actor.pos.x,
                actor.pos.y,
                actor.color,
                game_object::BACKGROUND_COLOR,
                &actor.tile,
            );
        }
    }

    fn draw_menu(menu: &Menu, console: &mut BTerm) {
        for (i, item) in menu.items.iter().enumerate() {
            let color: game_object::MenuColor;
            if i == menu.selected {
                color = game_object::MENU_COLOR_SELECTED;
            } else {
                color = game_object::MENU_COLOR_UNSELECTED;
            }
            console.print_color(
                menu.top_left.x,
                menu.top_left.y + i as i64,
                color.fg,
                color.bg,
                item,
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct Menu {
    top_left: game_object::Point,
    selected: usize,
    items: Arc<Vec<String>>,
}

pub enum MenuResult<'a> {
    None,
    Updated,
    Selected(&'a str),
    Back,
}

pub fn keydown_handler<'a>(keycode: Option<VirtualKeyCode>, menu: &'a mut Menu) -> MenuResult<'a> {
    match keycode {
        Some(VirtualKeyCode::Left) | Some(VirtualKeyCode::Up) => {
            menu.add(-1);
            MenuResult::Updated
        }
        Some(VirtualKeyCode::Right) | Some(VirtualKeyCode::Down) => {
            menu.add(1);
            MenuResult::Updated
        }
        Some(VirtualKeyCode::Space)
        | Some(VirtualKeyCode::NumpadEnter)
        | Some(VirtualKeyCode::Return) => MenuResult::Selected(&menu.items[menu.selected]),
        Some(VirtualKeyCode::Escape) => MenuResult::Back,
        _ => MenuResult::None,
    }
}

pub fn main_menu() -> Menu {
    static MAIN_MENU_ITEMS: LazyLock<Arc<Vec<String>>> = LazyLock::new(|| {
        Arc::new(vec![
            NEW_GAME.to_string(),
            LOAD_GAME.to_string(),
            "Placeholder 2".to_string(),
        ])
    });
    Menu {
        top_left: game_object::Point { x: 0, y: 0 },
        selected: 0,
        items: MAIN_MENU_ITEMS.clone(),
    }
}

impl Menu {
    pub fn add(&mut self, i: i64) {
        self.selected = (self.selected as i64 + i).rem_euclid(self.items.len() as i64) as usize
    }
}
