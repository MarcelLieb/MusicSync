use crate::utils::audioprocessing::Onset;

use super::LightService;
use colored::{ColoredString, Colorize};

#[derive(Debug, Default)]
pub struct Console {
    output: [ColoredString; 5],
}

impl LightService for Console {
    fn process_onset(&mut self, event: Onset) {
        match event {
            Onset::Kick(s) => self.output[0] = "■".repeat((s * 9.0).ceil() as usize).bright_red(),
            Onset::Hihat(s) => self.output[1] = "■".repeat((s * 9.0).ceil() as usize).white(),
            Onset::Full(s) => self.output[2] = "■".repeat((s * 9.0).ceil() as usize).cyan(),
            Onset::Note(s, _) => self.output[3] = "■".repeat((s * 9.0).ceil() as usize).blue(),
            Onset::Atmosphere(s, _) => {
                self.output[4] = "-".repeat((s * 9.0).ceil() as usize).black();
            }
            _ => {}
        }
    }

    fn update(&mut self) {
        print!("|  ");
        for s in self.output.iter().take(4) {
            print!("{s:^9}  |  ");
        }
        println!();
        for s in &mut self.output {
            *s = "".black();
        }
    }
}
