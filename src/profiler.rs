use rusqlite::named_params;

#[derive(Debug, Clone, Copy)]
pub struct TurnProfiler {}

impl TurnProfiler {
    pub fn new(db: &rusqlite::Connection) -> rusqlite::Result<Self> {
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS TurnSplit (
                turn INTEGER NOT NULL,
                split TEXT,
                ms INTEGER
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_turnsplit_turn_split ON TurnSplit (turn ASC, split);
            CREATE INDEX IF NOT EXISTS idx_turnsplit_split ON TurnSplit (split);
        ",
        )?;
        Ok(TurnProfiler {})
    }

    pub fn start(&mut self) -> TurnStart {
        let now = std::time::Instant::now();
        TurnStart {
            start: now,
            cut: now,
            splits: vec![],
        }
    }

    pub fn end(
        &mut self,
        db: &rusqlite::Connection,
        turn: i64,
        start: TurnStart,
        _actor_count: i64,
    ) -> rusqlite::Result<()> {
        let end = std::time::Instant::now();
        let duration = end.duration_since(start.start);
        let mut insert = db.prepare(
            "INSERT INTO TurnSplit (turn, split, ms)
            VALUES (:turn, :split, :ms)
            ON CONFLICT (turn, split) DO UPDATE SET turn = excluded.turn, split = excluded.split, ms = excluded.ms",
        )?;
        for (ref split, duration) in start.splits {
            insert.execute(
                named_params! {":turn": turn, ":split": split, ":ms": duration.as_millis() as u64},
            )?;
        }
        insert.execute(
            named_params! {":turn": turn, ":split": "complete", ":ms": duration.as_millis() as u64},
        )?;
        Ok(())
    }
}

pub struct TurnStart {
    start: std::time::Instant,
    cut: std::time::Instant,
    splits: std::vec::Vec<(String, std::time::Duration)>,
}

impl TurnStart {
    pub fn split(&mut self, segment: &str) {
        let cut = std::time::Instant::now();
        self.splits
            .push((segment.to_owned(), cut.duration_since(self.cut)));
        self.cut = cut;
    }
}
