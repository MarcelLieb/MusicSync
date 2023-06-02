use std::{fs::File, collections::HashMap};

use serde::{Serialize, Deserialize};
use ciborium::into_writer;

use super::{lights::{LightService, Event}, audiodevices::{SAMPLE_RATE, HOP_SIZE}};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct OnsetContainer {
    #[serde(skip_serializing, skip_deserializing)]
    filename: String,
    #[serde(skip_serializing, skip_deserializing)]
    time: u128,
    #[serde(skip_serializing, skip_deserializing)]
    time_interval: u32,
    pub data: HashMap<String, Vec<(u128, Event)>>
}

impl LightService for OnsetContainer {
    fn event_detected(&mut self, event: Event) {
        match event {
            Event::Full(_) => self.data.get_mut("Full").unwrap().push((self.time, event)),
            Event::Atmosphere(_, _) => self.data.get_mut("Atmosphere").unwrap().push((self.time, event)),
            Event::Note(_, _) => self.data.get_mut("Note").unwrap().push((self.time, event)),
            Event::Drum(_) => self.data.get_mut("Drum").unwrap().push((self.time, event)),
            Event::Hihat(_) => self.data.get_mut("Hihat").unwrap().push((self.time, event)),
        }
    }

    fn update(&mut self) {
        self.time = self.time + self.time_interval as u128;
    }
}

impl OnsetContainer {
    pub fn save(&self) -> std::io::Result<()> {
        let f = File::create(&self.filename)?;
        into_writer(&self.data, f).unwrap();
        Ok(())
    }

    pub fn init(filename: String) -> OnsetContainer {
        let data: HashMap<String, Vec<(u128, Event)>> = HashMap::from([
            ("Full".to_string(), Vec::new()),
            ("Atmosphere".to_string(), Vec::new()),
            ("Note".to_string(), Vec::new()),
            ("Drum".to_string(), Vec::new()),
            ("Hihat".to_string(), Vec::new()),
        ]);
        OnsetContainer {
            filename,
            time: 0,
            time_interval: ((HOP_SIZE as f64 / SAMPLE_RATE as f64) * 1000.0) as u32,
            data
        }
    }
}

impl Drop for OnsetContainer {
    fn drop(&mut self) {
        self.save().unwrap();
    }
}