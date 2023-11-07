use std::{collections::HashMap, fs::File};

use ciborium::into_writer;
use serde::{Deserialize, Serialize};

use super::{LightService, Onset};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct OnsetContainer {
    #[serde(skip_serializing, skip_deserializing)]
    filename: String,
    #[serde(skip_serializing, skip_deserializing)]
    time: u128,
    time_interval: u32,
    pub data: HashMap<String, Vec<(u128, Onset)>>,
    pub raw: Vec<f32>,
}

impl LightService for OnsetContainer {
    fn process_onset(&mut self, event: Onset) {
        match event {
            Onset::Full(_) => self.data.get_mut("Full").unwrap().push((self.time, event)),
            Onset::Atmosphere(_, _) => self
                .data
                .get_mut("Atmosphere")
                .unwrap()
                .push((self.time, event)),
            Onset::Note(_, _) => self.data.get_mut("Note").unwrap().push((self.time, event)),
            Onset::Drum(_) => self.data.get_mut("Drum").unwrap().push((self.time, event)),
            Onset::Hihat(_) => self.data.get_mut("Hihat").unwrap().push((self.time, event)),
            Onset::Raw(value) => self.raw.push(value),
        }
    }

    fn update(&mut self) {
        self.time += self.time_interval as u128;
    }
}

impl OnsetContainer {
    pub fn save(&self) -> std::io::Result<()> {
        let f = File::create(&self.filename)?;
        into_writer(&self, f).unwrap();
        Ok(())
    }

    pub fn init(filename: String, sample_rate: usize, hop_size: usize) -> OnsetContainer {
        let data: HashMap<String, Vec<(u128, Onset)>> = HashMap::from([
            ("Full".to_string(), Vec::new()),
            ("Atmosphere".to_string(), Vec::new()),
            ("Note".to_string(), Vec::new()),
            ("Drum".to_string(), Vec::new()),
            ("Hihat".to_string(), Vec::new()),
        ]);
        let raw = Vec::new();
        OnsetContainer {
            filename,
            time: 0,
            time_interval: ((hop_size as f64 / sample_rate as f64) * 1000.0) as u32,
            data,
            raw,
        }
    }
}

impl Drop for OnsetContainer {
    fn drop(&mut self) {
        match self.save() {
            Ok(_) => println!("Saved to {}", self.filename),
            Err(e) => println!("Error saving to {}: {}", self.filename, e),
        }
    }
}
