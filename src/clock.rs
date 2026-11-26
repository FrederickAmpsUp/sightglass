use web_time::Instant;

pub struct Clock {
    start_time: Instant,
    last_frame: Instant,
}

impl Clock {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_frame: now,
        }
    }

    pub fn tick(&mut self) -> (f32, f32) {
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_frame).as_secs_f32();
        let total_time = now.duration_since(self.start_time).as_secs_f32();
        self.last_frame = now;

        (total_time, delta_time)
    }
}
