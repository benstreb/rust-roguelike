use crate::{entity, game_object};

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

pub fn draw_actors(db: &rusqlite::Connection, console: &mut BTerm) -> Result<(), rusqlite::Error> {
    let mut conn = db.prepare("SELECT tile, x, y, r, g, b FROM Actor ORDER BY plane DESC")?;
    for row in conn.query_map((), |row| {
        let x: i64 = row.get("x")?;
        let y: i64 = row.get("y")?;
        let r: u8 = row.get("r")?;
        let g: u8 = row.get("g")?;
        let b: u8 = row.get("b")?;
        let tile: String = row.get("tile")?;
        Ok((x, y, bracket_lib::color::RGB::from_u8(r, g, b), tile))
    })? {
        let (x, y, foreground, tile) = row?;
        console.print_color(x, y, foreground, game_object::BACKGROUND_COLOR, tile);
    }
    Ok(())
}

#[derive(Debug)]
pub struct Menu {
    top_left: game_object::Point,
    selected: usize,
    items: Vec<String>,
}

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

pub fn main_menu() -> Menu {
    Menu {
        top_left: game_object::Point { x: 0, y: 0 },
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
                self.top_left.x,
                self.top_left.y + i as i64,
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
