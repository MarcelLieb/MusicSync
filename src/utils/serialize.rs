use std::fs::File;

use serde::Serialize;
use ciborium::into_writer;

use super::lights::{LightService, Event};

#[derive(Serialize, Debug, Default)]
pub struct OnsetContainer {
    #[serde(skip_serializing, skip_deserializing)]
    filename: String,
    time: u128,
    #[serde(skip_serializing, skip_deserializing)]
    current: TimeStep,
    data: Vec<TimeStep>
}

impl LightService for OnsetContainer {
    fn event_detected(&mut self, event: Event) {
        match event {
            Event::Full(s) => self.current.full = s,
            Event::Atmosphere(s, f) => self.current.atmosphere = (s, f),
            Event::Note(s, f) => self.current.note = (s, f),
            Event::Drum(s) => self.current.drum = s,
            Event::Hihat(s) => self.current.hihat = s,
        }
    }

    fn update(&mut self) {
        self.time = self.time + 1;
        self.data.push(self.current);
        self.current = TimeStep::default();
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
            current: TimeStep::default(),
            data: Vec::new()
        }
    }
}

impl Drop for OnsetContainer {
    fn drop(&mut self) {
        self.save().unwrap();
    }
}

#[derive(Debug, Default, Copy, Clone, Serialize)]
struct TimeStep {
    full: f32,
    atmosphere: (f32, u16),
    note: (f32, u16),
    drum: f32,
    hihat: f32,
}