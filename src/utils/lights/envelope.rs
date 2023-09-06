use std::time::{Duration, Instant};

use super::color::{hsv_to_rgb, interpolate_hsv, rgb_to_hsv};

pub trait Envelope {
    fn trigger(&mut self, strength: f32);
    fn get_value(&self) -> f32;
}
// Linear Envelope
pub struct FixedDecayEnvelope {
    trigger_time: Instant,
    length: Duration,
    strength: f32,
}

impl FixedDecayEnvelope {
    pub fn init(decay: std::time::Duration) -> FixedDecayEnvelope {
        FixedDecayEnvelope {
            trigger_time: Instant::now(),
            length: decay,
            strength: 0.0,
        }
    }
}

impl Envelope for FixedDecayEnvelope {
    fn trigger(&mut self, strength: f32) {
        self.trigger_time = Instant::now();
        self.strength = strength;
    }

    fn get_value(&self) -> f32 {
        let value = self.strength
            - (self.strength
                * (self.trigger_time.elapsed().as_millis() as f32
                    / self.length.as_millis() as f32));
        if value > 0.0 {
            value
        } else {
            0.0
        }
    }
}

pub struct DynamicDecayEnvelope {
    trigger_time: Instant,
    decay_per_second: f32,
    strength: f32,
}

impl DynamicDecayEnvelope {
    pub fn init(decay_per_second: f32) -> DynamicDecayEnvelope {
        DynamicDecayEnvelope {
            trigger_time: Instant::now(),
            decay_per_second,
            strength: 0.0,
        }
    }
}

impl Envelope for DynamicDecayEnvelope {
    fn trigger(&mut self, strength: f32) {
        self.trigger_time = Instant::now();
        self.strength = strength;
    }

    fn get_value(&self) -> f32 {
        let value = self.strength
            - (self.strength * self.trigger_time.elapsed().as_secs_f32() * self.decay_per_second);
        if value > 0.0 {
            value
        } else {
            0.0
        }
    }
}

#[allow(dead_code)]
pub struct ColorEnvelope {
    start_color: [f32; 3],
    end_color: [f32; 3],
    pub envelope: FixedDecayEnvelope,
}

#[allow(dead_code)]
impl ColorEnvelope {
    pub fn init(from_color: &[u16; 3], to_color: &[u16; 3], length: Duration) -> ColorEnvelope {
        ColorEnvelope {
            start_color: rgb_to_hsv(from_color),
            end_color: rgb_to_hsv(to_color),
            envelope: FixedDecayEnvelope::init(length),
        }
    }

    pub fn trigger(&mut self, strength: f32) {
        self.envelope.trigger(strength);
    }

    pub fn get_color(&self) -> [u16; 3] {
        let t = self.envelope.strength - self.envelope.get_value();
        hsv_to_rgb(&interpolate_hsv(&self.start_color, &self.end_color, t))
    }
}

#[allow(dead_code)]
pub struct AnimationHelper<T> {
    animator: fn(u64) -> T,
    time_ref: Instant,
    position: u64,
    length: u64,
    looping: bool,
    stopped: bool,
}

#[allow(dead_code)]
impl<T> AnimationHelper<T> {
    pub fn init(animator: fn(u64) -> T, length: u64, looping: bool) -> AnimationHelper<T> {
        AnimationHelper {
            animator,
            time_ref: Instant::now(),
            position: 0,
            length,
            looping,
            stopped: true,
        }
    }

    pub fn get_value(&self) -> T {
        let pos: u64;
        if self.stopped {
            pos = self.position;
        } else if self.looping {
            pos = (self.time_ref.elapsed().as_millis() % self.length as u128) as u64;
        } else if self.time_ref.elapsed().as_millis() > self.length as u128 {
            pos = self.length;
        } else {
            pos = self.time_ref.elapsed().as_millis() as u64;
        }
        (self.animator)(pos)
    }

    pub fn stop(&mut self) {
        self.position = (self.time_ref.elapsed().as_millis() % self.length as u128) as u64;
        self.stopped = true;
    }

    pub fn start(&mut self) {
        self.stopped = false;
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping
    }
}
