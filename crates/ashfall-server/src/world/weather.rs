//! Weather state — globally synced weather value.
//!
//! Clone shares the underlying value (Arc) — the server and every WASM
//! script instance observe the same authoritative weather.

use parking_lot::RwLock;
use std::sync::Arc;

/// Server-authoritative weather state.
pub struct WeatherState {
    weather: Arc<RwLock<u32>>,
}

impl Clone for WeatherState {
    fn clone(&self) -> Self {
        WeatherState {
            weather: self.weather.clone(),
        }
    }
}

impl WeatherState {
    pub fn new(initial: u32) -> Self {
        WeatherState {
            weather: Arc::new(RwLock::new(initial)),
        }
    }

    pub fn get(&self) -> u32 {
        *self.weather.read()
    }

    pub fn set(&self, value: u32) {
        *self.weather.write() = value;
    }
}

impl Default for WeatherState {
    fn default() -> Self {
        Self::new(0x00015E5E) // ponytail: Fallout3Clear default
    }
}
