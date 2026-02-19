//! Frame Clock
//!
//! Tracks delta time in seconds.
//! WASM-safe (no feature flags required)

pub struct Clock {
    last: f64,
    delta: f32,
}

impl Clock {
    pub fn new() -> Clock {
        let now = js_sys::Date::now();
        Clock {
            last: now,
            delta: 0.0,
        }
    }

    pub fn tick(&mut self) {
        let now = js_sys::Date::now();
        self.delta = ((now - self.last) / 1000.0) as f32;
        self.last = now;
    }

    pub fn delta_seconds(&self) -> f32 {
        self.delta
    }
}

