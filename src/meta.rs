use crate::{entity, game_object, menu};

use bracket_lib::terminal::BTerm;

pub const SAVE_FILE_NAME: &'static str = "game.db";

#[derive(Debug)]
pub enum GameMode {
    MainMenu(menu::Menu),
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
