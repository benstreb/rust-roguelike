use crate::{component, console, game_object, meta, system};

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
        gamemode: &meta::GameMode,
        console: &mut console::Console,
        ctx: &mut ggez::Context,
    ) -> rusqlite::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        console.cls(ctx);
        match gamemode {
            meta::GameMode::MainMenu(menu) => Self::draw_menu(menu, console),
            meta::GameMode::InGame {
                db, selected_point, ..
            } => {
                let visible_objects = system::get_visible(db)?;
                Self::draw_objects(&visible_objects, console);
                let turn = component::player::turns_passed(db)?;
                console.print(console::ConsolePoint { x: 0, y: 0 }, &turn.to_string());
                if let Some(pos) = selected_point {
                    console.print(
                        console::ConsolePoint {
                            x: 0,
                            y: meta::WORLD_HEIGHT + 2,
                        },
                        &format!("({:<2}, {:<2})", pos.x, pos.y),
                    );
                } else {
                    console.print(
                        console::ConsolePoint {
                            x: 0,
                            y: meta::WORLD_HEIGHT + 2,
                        },
                        "Click something!",
                    );
                }
            }
            meta::GameMode::WonGame => {
                console.cls(ctx);
                console.print(console::ConsolePoint { x: 1, y: 1 }, "You Win");
            }
        }
        console.finish(ctx).expect("I'm dead!");
        self.dirty = false;
        Ok(())
    }

    fn draw_objects(objects: &Vec<game_object::Object>, console: &mut console::Console) {
        for object in objects {
            console.print_color(
                object.pos.into(),
                object.tile.color,
                game_object::BACKGROUND_COLOR,
                &object.tile.tile,
            );
        }
    }

    fn draw_menu(menu: &meta::Menu, console: &mut console::Console) {
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
