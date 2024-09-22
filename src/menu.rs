use bracket_lib::terminal::{BTerm, VirtualKeyCode};

use crate::game_object;

pub enum MenuResult<'a> {
    None,
    Selected(&'a str),
    Back,
}

pub fn keydown_handler<'a>(keycode: Option<VirtualKeyCode>, menu: &'a mut Menu) -> MenuResult<'a> {
    match keycode {
        Some(VirtualKeyCode::Left) | Some(VirtualKeyCode::Up) => {
            menu.add(-1);
        }
        Some(VirtualKeyCode::Right) | Some(VirtualKeyCode::Down) => {
            menu.add(1);
        }
        Some(VirtualKeyCode::Space)
        | Some(VirtualKeyCode::NumpadEnter)
        | Some(VirtualKeyCode::Return) => {
            return MenuResult::Selected(&menu.items[menu.selected]);
        }
        Some(VirtualKeyCode::Escape) => {
            return MenuResult::Back;
        }
        _ => {}
    };
    MenuResult::None
}

#[derive(Debug)]
pub struct Menu {
    top_left_x: i64,
    top_left_y: i64,
    selected: usize,
    items: Vec<String>,
}

pub const NEW_GAME: &str = "New Game";
pub const LOAD_GAME: &str = "Load Game";

pub fn main_menu() -> Menu {
    Menu {
        top_left_x: 0,
        top_left_y: 0,
        selected: 0,
        items: vec![
            NEW_GAME.to_string(),
            LOAD_GAME.to_string(),
            "Placeholder 2".to_string(),
        ],
    }
}

impl Menu {
    pub fn draw(&self, console: &mut BTerm) -> () {
        for (i, item) in self.items.iter().enumerate() {
            let color: game_object::MenuColor;
            if i == self.selected {
                color = game_object::MENU_COLOR_SELECTED;
            } else {
                color = game_object::MENU_COLOR_UNSELECTED;
            }
            console.print_color(
                self.top_left_x,
                self.top_left_y + i as i64,
                color.fg,
                color.bg,
                item,
            )
        }
    }

    pub fn add(&mut self, i: i64) {
        self.selected = (self.selected as i64 + i).rem_euclid(self.items.len() as i64) as usize
    }
}
