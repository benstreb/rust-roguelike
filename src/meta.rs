use crate::{entity, menu};

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
