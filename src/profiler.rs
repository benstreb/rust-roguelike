use std::io::Write;

pub struct TurnProfiler {
    file: std::fs::File,
}

impl TurnProfiler {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<Self> {
        let mut file = std::fs::File::create(path)?;
        writeln!(file, "turn,time (ms),actors")?;
        std::io::Result::Ok(TurnProfiler { file })
    }

    pub fn start(&mut self) -> TurnStart {
        TurnStart {
            start: std::time::Instant::now(),
        }
    }

    pub fn end(&mut self, turn: i64, start: TurnStart, actor_count: i64) -> std::io::Result<()> {
        let end = std::time::Instant::now();
        let duration = end.duration_since(start.start);
        let ms = duration.as_millis();
        writeln!(self.file, "{},{},{}", turn, ms, actor_count)
    }
}

pub struct TurnStart {
    start: std::time::Instant,
}
