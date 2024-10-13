use std::sync::Arc;

use log::{info, warn};
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use tokio::sync::{broadcast, oneshot};

use crate::{nodes::{internal::Getters, NodeTrait, CHANNEL_SIZE}, utils::audioprocessing::{window, WindowType}};



pub struct FFT {
    sender: broadcast::Sender<Arc<[f32]>>,
    receiver: Option<broadcast::Receiver<Arc<[f32]>>>,
    handle: Option<tokio::task::JoinHandle<(Vec<Complex<f32>>, Vec<Complex<f32>>)>>,
    stop_signal: Option<oneshot::Sender<()>>,
    fft_planner: Arc<dyn RealToComplex<f32>>,
    fft_size: usize,
    output_buffer: Option<Vec<Complex<f32>>>,
    scratch_buffer: Option<Vec<Complex<f32>>>,
    window: Arc<[f32]>,
}

impl FFT {
    pub fn init(fft_size: usize, window_type: WindowType) -> Self {
        let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(fft_size as usize);
        let output_buffer = fft_planner.make_output_vec().into();
        let scratch_buffer = fft_planner.make_scratch_vec().into();
        let window = window(fft_size, window_type).into();
        let (sender, _) = broadcast::channel(CHANNEL_SIZE);
        Self {
            sender,
            receiver: None,
            handle: None,
            stop_signal: None,
            fft_planner,
            fft_size,
            window,
            output_buffer,
            scratch_buffer,
        }
    }
}

impl Getters<Arc<[f32]>, Arc<[f32]>, (Vec<Complex<f32>>, Vec<Complex<f32>>)> for FFT {
    fn get_sender(&self) -> &broadcast::Sender<Arc<[f32]>> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<Arc<[f32]>>> {
        &mut self.receiver
    }
    
    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<(Vec<Complex<f32>>, Vec<Complex<f32>>)>> {
        &mut self.handle
    }
}

impl FFT {
    async fn stop_task(&mut self) {
        if let Some(stop_signal) = self.stop_signal.take() {
            stop_signal.send(()).ok();
        }
        if let Some(handle) = self.handle.take() {
            if let Some((out, scratch)) = handle.await.ok() {
                self.output_buffer.replace(out);
                self.scratch_buffer.replace(scratch);
            }
        }
    }
}

impl NodeTrait<Arc<[f32]>, Arc<[f32]>, (Vec<Complex<f32>>, Vec<Complex<f32>>)> for FFT {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, Arc<[f32]>, F>) {
        self.stop_task().await;

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_signal.replace(stop_tx);

        let sender = self.sender.clone();
        let mut receiver = node.subscribe();
        let fft_planner = self.fft_planner.clone();
        let fft_size = self.fft_size;
        let mut out_buffer = if self.output_buffer.is_none() {
            fft_planner.make_output_vec().into()
        } else {
            self.output_buffer.take().unwrap()
        };
        let mut scratch_buffer = if self.scratch_buffer.is_none() {
            fft_planner.make_scratch_vec().into()
        } else {
            self.scratch_buffer.take().unwrap()
        };
        let window = self.window.clone();

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = stop_rx => {(out_buffer, scratch_buffer)},
                _ = async {
                    loop {
                        match receiver.recv().await {
                            Ok(data) => {
                                if data.len() != fft_size {
                                    warn!("Data length of {} does not match FFT size of {}. Skipping.", data.len(), fft_size);
                                    continue;
                                }
                                let mut data = data.iter().zip(window.iter()).map(|(a, b)| a * b).collect::<Vec<f32>>();
                                let status = fft_planner.process_with_scratch(&mut data, &mut out_buffer, &mut scratch_buffer);

                                if status.is_err() {
                                    warn!("FFT failed. Skipping.");
                                    continue;
                                }

                                let mut status = sender.send(out_buffer.iter().map(|c| c.norm()).collect::<Vec<f32>>().into());
                                while status.is_err() {
                                    tokio::task::yield_now().await;
                                    status = sender.send(status.err().unwrap().0);
                                }
                            },
                            Err(e) => match e {
                                broadcast::error::RecvError::Closed => warn!("Sender closed"),
                                broadcast::error::RecvError::Lagged(n) => info!("Lagged: {}", n),
                            },
                        }
                    }
                } => {(out_buffer, scratch_buffer)},
            }
        });

        self.handle.replace(handle);
    }

    async fn unfollow(&mut self) {
        self.stop_task().await;
    }
}