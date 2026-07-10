//! Weather state — globally synced weather value.

use parking_lot::RwLock;

/// Server-authoritative weather state.
pub struct WeatherState {
    weather: RwLock<u32>,
}

impl Clone for WeatherState {
    fn clone(&self) -> Self {
        WeatherState {
            weather: RwLock::new(*self.weather.read()),
        }
    }
}

impl WeatherState {
    pub fn new(initial: u32) -> Self {
        WeatherState {
            weather: RwLock::new(initial),
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
