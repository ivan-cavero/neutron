//! Dedicated worldgen thread + chunk cache.
//!
//! `ChunkGenerator` is `!Send` (`Rc` density tree). One OS thread owns the
//! generator; login/tick ask it for encoded chunks through a channel.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::chunk_sender;

/// A fully encoded chunk ready to put on the wire (without X/Z).
#[derive(Clone)]
pub struct EncodedChunk {
    /// Bytes after the chunk X/Z ints: heightmaps + sections + light.
    pub body: Vec<u8>,
    /// Highest solid Y per column (index = z * 16 + x).
    pub heightmap: Vec<i16>,
}

enum Request {
    Chunk {
        cx: i32,
        cz: i32,
        resp: mpsc::Sender<Arc<EncodedChunk>>,
    },
    Spawn {
        resp: mpsc::Sender<(f64, f64, f64)>,
    },
}

/// Handle to the worldgen worker. Cheap to clone.
#[derive(Clone)]
pub struct WorldgenHandle {
    tx: mpsc::Sender<Request>,
}

impl WorldgenHandle {
    /// Start the worker for `seed`. Blocks the worker on first spawn-height
    /// compute (one chunk) so login can query it immediately after.
    pub fn start(seed: i64) -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        thread::Builder::new()
            .name("neutron-worldgen".into())
            .spawn(move || worker(seed, rx))
            .expect("worldgen thread");
        Self { tx }
    }

    /// Generate (or fetch cached) chunk. Safe to call from any thread.
    pub fn chunk(&self, cx: i32, cz: i32) -> Arc<EncodedChunk> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.tx
            .send(Request::Chunk {
                cx,
                cz,
                resp: resp_tx,
            })
            .expect("worldgen worker alive");
        resp_rx.recv().expect("worldgen worker response")
    }

    /// Spawn feet position on top of the column at world (0, 0).
    pub fn spawn_xyz(&self) -> (f64, f64, f64) {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.tx
            .send(Request::Spawn { resp: resp_tx })
            .expect("worldgen worker alive");
        resp_rx.recv().expect("worldgen worker spawn")
    }

    /// Async wrapper so the tokio runtime is not blocked by generation.
    pub async fn chunk_async(&self, cx: i32, cz: i32) -> Arc<EncodedChunk> {
        let handle = self.clone();
        tokio::task::spawn_blocking(move || handle.chunk(cx, cz))
            .await
            .expect("worldgen spawn_blocking")
    }

    pub async fn spawn_xyz_async(&self) -> (f64, f64, f64) {
        let handle = self.clone();
        tokio::task::spawn_blocking(move || handle.spawn_xyz())
            .await
            .expect("worldgen spawn_blocking")
    }
}

fn worker(seed: i64, rx: mpsc::Receiver<Request>) {
    tracing::info!(seed, "worldgen worker starting (building NoiseRouter)");
    let started = std::time::Instant::now();
    let gen = neutron_worldgen::ChunkGenerator::new(seed);
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "worldgen worker ready"
    );

    let mut cache: HashMap<(i32, i32), Arc<EncodedChunk>> = HashMap::new();
    let mut spawn: Option<(f64, f64, f64)> = None;

    while let Ok(req) = rx.recv() {
        match req {
            Request::Chunk { cx, cz, resp } => {
                let encoded = cache
                    .entry((cx, cz))
                    .or_insert_with(|| {
                        let t0 = std::time::Instant::now();
                        let chunk = gen.generate_chunk(cx, cz);
                        let body = chunk_sender::encode_playable_chunk(&chunk);
                        tracing::debug!(
                            cx,
                            cz,
                            ms = t0.elapsed().as_millis() as u64,
                            bytes = body.len(),
                            "generated chunk"
                        );
                        Arc::new(EncodedChunk {
                            body,
                            heightmap: chunk.heightmap,
                        })
                    })
                    .clone();
                let _ = resp.send(encoded);
            }
            Request::Spawn { resp } => {
                if spawn.is_none() {
                    let encoded = cache.entry((0, 0)).or_insert_with(|| {
                        let chunk = gen.generate_chunk(0, 0);
                        let body = chunk_sender::encode_playable_chunk(&chunk);
                        Arc::new(EncodedChunk {
                            body,
                            heightmap: chunk.heightmap,
                        })
                    });
                    // Column (0, 0) inside chunk (0, 0) is index 0.
                    let top = encoded.heightmap.first().copied().unwrap_or(64);
                    let y = f64::from(top) + 1.0;
                    spawn = Some((0.5, y, 0.5));
                    tracing::info!(y, "computed spawn from heightmap at (0, 0)");
                }
                let _ = resp.send(spawn.unwrap());
            }
        }
    }
}
