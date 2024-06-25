use bracket_lib::prelude::{main_loop, BResult, BTerm, BTermBuilder, GameState};
use rusqlite::Connection;

struct State {}
impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        ctx.cls();
        ctx.print(1, 1, "Hello Rust World");
    }
}

fn main() -> BResult<()> {
    let conn = Connection::open_in_memory()?;

    conn.execute(
        "CREATE TABLE person (
            id    INTEGER PRIMARY KEY,
            name  TEXT NOT NULL,
            data  BLOB
        )",
        (), // empty list of parameters.
    )?;

    let context = BTermBuilder::simple80x50()
        .with_title("Hello Rust World")
        .build()?;
    let gs = State {};
    main_loop(context, gs)
}
