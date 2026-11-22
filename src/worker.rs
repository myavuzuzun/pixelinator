use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use image::RgbaImage;

use crate::pipeline::PipelineCache;
use crate::state::AppSettings;

pub struct WorkerRequest {
    pub job_id: u64,
    pub source_id: u64,
    pub source: Arc<RgbaImage>,
    pub settings: AppSettings,
    pub immediate: bool,
}

pub struct WorkerResponse {
    pub job_id: u64,
    pub image: RgbaImage,
}

pub struct PipelineWorker {
    tx: Sender<WorkerRequest>,
    rx: Receiver<WorkerResponse>,
}

impl PipelineWorker {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<WorkerRequest>();
        let (res_tx, res_rx) = mpsc::channel::<WorkerResponse>();

        thread::spawn(move || {
            let mut cache = PipelineCache::new();

            while let Ok(mut latest) = req_rx.recv() {
                if !latest.immediate {
                    let deadline = Instant::now() + Duration::from_millis(80);
                    loop {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            break;
                        }
                        match req_rx.recv_timeout(remaining) {
                            Ok(newer) => {
                                latest = newer;
                                if latest.immediate {
                                    break;
                                }
                            }
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                }

                // Drain any requests that arrived while waiting so latest wins.
                while let Ok(newer) = req_rx.try_recv() {
                    latest = newer;
                }

                let result = cache.process(latest.source_id, &latest.source, &latest.settings);
                if res_tx
                    .send(WorkerResponse {
                        job_id: latest.job_id,
                        image: result,
                    })
                    .is_err()
                {
                    return;
                }
                ctx.request_repaint();
            }
        });

        Self {
            tx: req_tx,
            rx: res_rx,
        }
    }

    pub fn submit(&self, req: WorkerRequest) {
        let _ = self.tx.send(req);
    }

    pub fn poll(&self) -> Option<WorkerResponse> {
        let mut latest = None;
        while let Ok(resp) = self.rx.try_recv() {
            latest = Some(resp);
        }
        latest
    }
}
