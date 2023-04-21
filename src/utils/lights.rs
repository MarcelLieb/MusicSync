use std::marker::Send;
use std::sync::mpsc;
use std::time::{Instant, Duration};
use std::{
    sync::{
        mpsc::{Sender, TryRecvError},
        Arc, Mutex,
    },
    thread,
};

use futures::executor::block_on;
use webrtc_dtls::conn::DTLSConn;

pub trait LightService {
    fn event_detected(&mut self, event: Event);
    fn update(&mut self);
}

pub trait Closable {
    fn close_connection(&self);
}

pub trait Writeable {
    fn write_buffer(&self, buffer: &[u8]);
}

pub trait Stream: Writeable + Closable + Send {}

pub enum Event {
    Full(f32),
    Atmosphere(u16, f32),
    Note(u16, f32),
    Drum(f32),
    Hihat(f32),
}

struct Poller<T: Stream + Send + Sync> {
    stream: T,
}

impl<T: Stream + Send + Sync> Poller<T> {
    fn poll(&self, bytes: &[u8]) {
        self.stream.write_buffer(bytes);
    }
}

impl<T: Stream + Send + Sync> Closable for Arc<Poller<T>> {
    fn close_connection(&self) {
        self.stream.close_connection()
    }
}

impl<T: Stream + Send + Sync> Writeable for Arc<Poller<T>> {
    fn write_buffer(&self, buffer: &[u8]) {
        self.stream.write_buffer(buffer)
    }
}

impl<T: Stream + Send + Sync> Stream for Arc<Poller<T>> {}

pub struct PollingHelper<T: Stream + Send + Sync> {
    colors: Arc<Mutex<Vec<[u16; 3]>>>,
    pub polling_frequency: u16,
    poller: Arc<Poller<T>>,
    tx: Sender<bool>,
}

impl<T: Stream + Send + Sync + 'static> PollingHelper<T> {
    pub fn init<F>(
        stream: Arc<T>,
        formatter: Arc<dyn Fn(&[[u16; 3]]) -> Vec<u8> + Send + Sync>,
        polling_frequency: u16,
    ) -> PollingHelper<Arc<T>>
    where
        Arc<T>: Stream,
    {
        let poller = Arc::new(Poller { stream });
        let colors: Vec<[u16; 3]> = vec![[0, 0, 0]];
        let colors = Arc::new(Mutex::new(colors));
        let format = formatter;
        let poller_rc = poller.clone();
        let colors_rc = colors.clone();

        let (tx, rx) = mpsc::channel::<bool>();

        thread::spawn(move || loop {
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    break;
                }
                Err(TryRecvError::Empty) => {}
            }
            let colors = colors_rc.lock().unwrap();
            let bytes = format(&colors);
            poller_rc.poll(&bytes);
            drop(colors);
            thread::sleep(std::time::Duration::from_millis(
                (1000 / polling_frequency) as u64,
            ));
        });
        return PollingHelper {
            colors,
            polling_frequency,
            poller,
            tx,
        };
    }

    pub fn update_color(&mut self, colors: &[[u16; 3]], additive: bool) {
        let mut colors_lock = self.colors.lock().unwrap();
        let size = colors_lock.len();
        if colors.len() > size {
            colors_lock.extend(std::iter::repeat([0, 0, 0]).take(colors.len() - size))
        }
        if additive {
            colors_lock
                .iter_mut()
                .zip(colors.iter())
                .for_each(|(old, new)| {
                    (0..3).for_each(|i| old[i] = old[i].saturating_add(new[i]));
                });
        } else {
            colors_lock
                .iter_mut()
                .zip(colors.iter())
                .for_each(|(old, new)| {
                    (0..3).for_each(|i| old[i] = new[i]);
                });
        }
    }
}

impl<T: Stream + Send + Sync> Drop for PollingHelper<T> {
    fn drop(&mut self) {
        self.tx.send(true).unwrap();
        self.poller.close_connection();
    }
}

impl Writeable for DTLSConn {
    fn write_buffer(&self, buffer: &[u8]) {
        block_on(self.write(buffer, None)).unwrap();
    }
}

impl Closable for DTLSConn {
    fn close_connection(&self) {
        block_on(self.close()).unwrap();
    }
}

impl Stream for DTLSConn {}

impl Writeable for Arc<DTLSConn> {
    fn write_buffer(&self, buffer: &[u8]) {
        block_on(self.write(buffer, None)).unwrap();
    }
}

impl Closable for Arc<DTLSConn> {
    fn close_connection(&self) {
        block_on(self.close()).unwrap();
    }
}

impl Stream for Arc<DTLSConn> {}

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
        return FixedDecayEnvelope {
            trigger_time: Instant::now(),
            length: decay,
            strength: 0.0
        };
    }
}

impl Envelope for FixedDecayEnvelope {
    fn trigger(&mut self, strength: f32) {
        self.trigger_time = Instant::now();
        self.strength = strength;
    }

    fn get_value(&self) -> f32 {
        let value = self.strength - (self.strength * (self.trigger_time.elapsed().as_millis() as f32 / self.length.as_millis() as f32));
        return if value > 0.0 { value } else { 0.0 };
    }
}

pub struct DynamicDecayEnvelope {
    trigger_time: Instant,
    decay_per_second: f32,
    strength: f32,
}

impl DynamicDecayEnvelope {
    pub fn init(decay_per_second: f32) -> DynamicDecayEnvelope {
        return DynamicDecayEnvelope {
            trigger_time: Instant::now(),
            decay_per_second: decay_per_second,
            strength: 0.0
        };
    }
}

impl Envelope for DynamicDecayEnvelope {
    fn trigger(&mut self, strength: f32) {
        self.trigger_time = Instant::now();
        self.strength = strength;
    }

    fn get_value(&self) -> f32 {
        let value = self.strength - (self.strength * self.trigger_time.elapsed().as_secs_f32() * self.decay_per_second);
        return if value > 0.0 { value } else { 0.0 };
    }
}

pub struct ColorEnvelope {
    start_color: [f32; 3],
    end_color: [f32; 3],
    envelope: FixedDecayEnvelope,
}

impl ColorEnvelope {
    pub fn init(from_color: &[u16; 3], to_color: &[u16; 3], length: Duration) -> ColorEnvelope {
        return ColorEnvelope {
            start_color: rgb_to_xyb(from_color),
            end_color: rgb_to_xyb(to_color),
            envelope: FixedDecayEnvelope::init(length)
        };
    }

    pub fn trigger(&mut self, strength: f32) {
        self.envelope.trigger(strength);
    }

    pub fn get_color(&self) -> [u16; 3] {
        let t = self.envelope.get_value();
        let x = self.start_color[0] + (self.end_color[0] - self.start_color[0]) * t;
        let y = self.start_color[1] + (self.end_color[1] - self.start_color[1]) * t;
        return  xyb_to_rgb(&[x, y, t]);
    }
}

pub struct MultibandEnvelope {
    pub drum: DynamicDecayEnvelope,
    pub hihat: FixedDecayEnvelope,
    pub note: ColorEnvelope,
    pub fullband: FixedDecayEnvelope,
}

#[allow(non_snake_case)]
pub fn rgb_to_xyb(rgb: &[u16; 3]) -> [f32; 3] {
    let mut rgb: [f32; 3] = rgb
        .iter()
        .map(|v| *v as f32 / u16::MAX as f32)
        .collect::<Vec<f32>>()
        .try_into()
        .unwrap();
    rgb
        .iter_mut()
        .for_each(|v| *v = if *v > 0.04045 {((*v + 0.055) / 1.055).powf(2.4)} else {*v / 12.92});

    let X = rgb[0] * 0.4124 + rgb[1] * 0.3576 + rgb[2] * 0.1805;
    let Y = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    let Z = rgb[0] * 0.0193 + rgb[1] * 0.1192 + rgb[2] * 0.9505;

    let x = X / (X + Y + Z);
    let y = Y / (X + Y + Z);

    return [x, y, Y]
}

#[allow(non_snake_case)]
pub fn xyb_to_rgb(xyb: &[f32; 3]) -> [u16; 3] {
    let x = xyb[0];
    let y = xyb[1];
    let z = 1.0 - x - y;
    let Y = xyb[2];
    let X = (Y / y) * x;
    let Z = (Y / y) * z;
    let mut r = X * 3.2406 - Y * 1.537 - Z * 0.4986;
    let mut g = -X * 0.9689 + Y * 1.8758 + Z * 0.0415;
    let mut b = X * 0.0557 - Y * 0.2040 + Z * 1.0570;
    r = if r <= 0.0031308 {12.92 * r} else {(1.0 + 0.055) * r.powf(1.0 / 2.4) - 0.055};
    g = if g <= 0.0031308 {12.92 * g} else {(1.0 + 0.055) * g.powf(1.0 / 2.4) - 0.055};
    b = if b <= 0.0031308 {12.92 * b} else {(1.0 + 0.055) * b.powf(1.0 / 2.4) - 0.055};
    return [(r * u16::MAX as f32) as u16, (g * u16::MAX as f32) as u16, (b * u16::MAX as f32) as u16];
}