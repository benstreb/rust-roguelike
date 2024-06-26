use bracket_lib::terminal::BTerm;

pub fn draw_actors(
    conn: &rusqlite::Connection,
    console: &mut BTerm,
) -> Result<(), rusqlite::Error> {
    let mut conn = conn.prepare("SELECT tile, x, y FROM Actor ORDER BY plane DESC")?;
    for row in conn.query_map((), |row| {
        let x: i64 = row.get("x")?;
        let y: i64 = row.get("y")?;
        let tile: String = row.get("tile")?;
        Ok((x, y, tile))
    })? {
        let (x, y, tile) = row?;
        console.print(x, y, tile);
    }
    Ok(())
}
