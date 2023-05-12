use std::fs::File;

use serde::Serialize;
use ciborium::into_writer;

use super::lights::{LightService, Event};

#[derive(Serialize, Debug, Default)]
pub struct OnsetContainer {
    #[serde(skip_serializing, skip_deserializing)]
    filename: String,
    #[serde(skip_serializing, skip_deserializing)]
    time: u128,
    data: Vec<(u128, Event)>
}

impl LightService for OnsetContainer {
    fn event_detected(&mut self, event: Event) {
        self.data.push((self.time, event))
    }

    fn update(&mut self) {
        self.time = self.time + 1;
    }
}

impl OnsetContainer {
    pub fn save(&self) -> std::io::Result<()> {
        let f = File::create(&self.filename)?;
        into_writer(self, f).unwrap();
        Ok(())
    }

    pub fn init(filename: String) -> OnsetContainer {
        OnsetContainer {
            filename,
            time: 0,
            data: Vec::new()
        }
    }
}

impl Drop for OnsetContainer {
    fn drop(&mut self) {
        self.save().unwrap();
    }
}