use super::{LightService, Event};
use colored::{ColoredString, Colorize};


#[derive(Debug, Default)]
pub struct Console {
    output: [ColoredString; 5],
}

impl LightService for Console {
    fn event_detected(&mut self, event: Event) {
        match event {
            Event::Drum(s) => self.output[0] = "■".repeat((s * 9.0).ceil() as usize).bright_red(),
            Event::Hihat(s) => self.output[1] = "■".repeat((s * 9.0).ceil() as usize).white(),
            Event::Full(s) => self.output[2] = "■".repeat((s * 9.0).ceil() as usize).cyan(),
            Event::Note(s, _) => self.output[3] = "■".repeat((s * 9.0).ceil() as usize).blue(),
            Event::Atmosphere(s, _) => {
                self.output[4] = "-".repeat((s * 9.0).ceil() as usize).black()
            }
            _ => {}
        }
    }

    fn update(&mut self) {
        print!("|  ");
        for s in self.output.iter().take(4) {
            print!("{:^9}  |  ", s);
        }
        println!();
        for s in &mut self.output {
            *s = "".black();
        }
    }
}