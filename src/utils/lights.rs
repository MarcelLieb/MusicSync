use std::marker::Send;
use std::sync::mpsc;
use std::time::Instant;
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

// Linear Envelope
pub struct Envelope {
    trigger_time: Instant,
    length: std::time::Duration,
    strength: f32,
}

impl Envelope {
    pub fn init(decay: std::time::Duration) -> Envelope {
        return Envelope {
            trigger_time: Instant::now(),
            length: decay,
            strength: 0.0
        };
    }

    pub fn trigger(&mut self, strength: f32) {
        self.trigger_time = Instant::now();
        self.strength = strength;
    }

    pub fn get_value(&self) -> f32 {
        let value = self.strength - (self.strength * ((Instant::now() - self.trigger_time).as_millis() / self.length.as_millis()) as f32);
        return if value > 0.0 { value } else { 0.0 };
    }
}


pub struct MultibandEnvelope {
    pub drum: Envelope,
    pub hihat: Envelope,
    pub note: Envelope,
    pub fullband: Envelope,
}