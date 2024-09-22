use crate::{entity, menu};

#[derive(Debug)]
pub enum GameMode {
    MainMenu(menu::Menu),
    InGame {
        db: rusqlite::Connection,
        player: entity::Entity,
    },
    WonGame,
}
