use std::time::Instant;

pub(super) struct Stopwatch {
    mark: Instant,
    pub(super) laps: Vec<(&'static str, f64)>,
}

impl Stopwatch {
    pub(super) fn start() -> Self {
        Self {
            mark: Instant::now(),
            laps: Vec::new(),
        }
    }

    pub(super) fn lap(&mut self, name: &'static str) {
        self.laps
            .push((name, self.mark.elapsed().as_secs_f64() * 1000.0));
        self.mark = Instant::now();
    }
}
