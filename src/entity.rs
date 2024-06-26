use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

#[derive(Clone, Copy, Debug)]
pub struct Entity {
    id: i64,
}

impl ToSql for Entity {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok(self.id.into())
    }
}

impl FromSql for Entity {
    fn column_result(value: ValueRef) -> FromSqlResult<Self> {
        Ok(Entity {
            id: value.as_i64()?,
        })
    }
}

pub fn create_table(sql: &rusqlite::Connection) -> rusqlite::Result<()> {
    sql.execute_batch("CREATE TABLE Entity (id INTEGER PRIMARY KEY)")
}

pub fn create(sql: &rusqlite::Connection) -> rusqlite::Result<Entity> {
    sql.query_row("INSERT INTO Entity VALUES (NULL) RETURNING id", (), |row| {
        Ok(Entity { id: row.get(0)? })
    })
}
