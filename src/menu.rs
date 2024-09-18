use bracket_lib::terminal::{VirtualKeyCode, BTerm};

use crate::game_object;

pub fn keydown_handler(
    keycode: Option<VirtualKeyCode>,
    menu: &mut Option<Menu>,
) -> rusqlite::Result<()> {
    let Some(ref mut m) = menu else {panic!("expected menu to be some in handler")};
    match keycode {
        Some(VirtualKeyCode::Left) | Some(VirtualKeyCode::Up)=> {
            m.add(-1);
        }
        Some(VirtualKeyCode::Right) | Some(VirtualKeyCode::Down) => {
            m.add(1);
        }
        Some(VirtualKeyCode::Space) | Some(VirtualKeyCode::NumpadEnter) => {
            dbg!("I'm not trapped in here with you, you're trapped in here with me!");
        }
        _ => {}
    };
    Ok(())
}



#[derive(Debug)]
pub struct Menu {
    top_left_x: i64,
    top_left_y: i64,
    selected: usize,
    items: Vec<String>,
}

pub fn main_menu() -> Menu {
    Menu {
        top_left_x: 0,
        top_left_y: 0,
        selected: 0,
        items: vec![
            "New Game".to_string(),
            "Placeholder 1".to_string(),
            "Placeholder 2".to_string(),
        ]
    }
}

impl Menu {
    pub fn draw(
        &self,
        console: &mut BTerm,
    ) -> () {
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
