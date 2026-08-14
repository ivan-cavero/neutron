//! Dedicated worldgen thread + chunk cache.
//!
//! `ChunkGenerator` is `!Send` (`Rc` density tree). One OS thread owns the
//! generator; login/tick ask it for encoded chunks through a channel.
//!
//! Pre-feature noise+surface is cached so neighbouring 3×3 decorations
//! do not redo the expensive density fill.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use neutron_worldgen::NoiseCache;

use crate::chunk_sender;

/// Soft cap so a long-lived process does not keep every column forever.
const READY_CACHE_CAP: usize = 512;
const NOISE_CACHE_CAP: usize = 256;

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
    Prefetch {
        cx: i32,
        cz: i32,
    },
    Spawn {
        resp: mpsc::Sender<(f64, f64, f64)>,
    },
}

/// Handle to the worldgen worker. Cheap to clone.
#[derive(Clone)]
pub struct WorldgenHandle {
    tx: mpsc::Sender<Request>,
    ready: Arc<Mutex<HashMap<(i32, i32), Arc<EncodedChunk>>>>,
    inflight: Arc<Mutex<HashSet<(i32, i32)>>>,
}

impl WorldgenHandle {
    /// Start the worker for `seed`.
    pub fn start(seed: i64) -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let ready = Arc::new(Mutex::new(HashMap::new()));
        let inflight = Arc::new(Mutex::new(HashSet::new()));
        let ready_worker = ready.clone();
        let inflight_worker = inflight.clone();
        thread::Builder::new()
            .name("neutron-worldgen".into())
            .spawn(move || worker(seed, rx, ready_worker, inflight_worker))
            .expect("worldgen thread");
        Self {
            tx,
            ready,
            inflight,
        }
    }

    /// Generate (or fetch cached) chunk. Safe to call from any thread.
    pub fn chunk(&self, cx: i32, cz: i32) -> Arc<EncodedChunk> {
        if let Some(hit) = self.try_chunk(cx, cz) {
            return hit;
        }
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

    /// Cached chunk, if the worker has already finished it.
    pub fn try_chunk(&self, cx: i32, cz: i32) -> Option<Arc<EncodedChunk>> {
        self.ready.lock().ok()?.get(&(cx, cz)).cloned()
    }

    /// Ask the worker to generate this column when idle (does not block).
    pub fn prefetch(&self, cx: i32, cz: i32) {
        if self.try_chunk(cx, cz).is_some() {
            return;
        }
        if let Ok(mut inf) = self.inflight.lock() {
            if !inf.insert((cx, cz)) {
                return;
            }
        }
        if self.tx.send(Request::Prefetch { cx, cz }).is_err() {
            if let Ok(mut inf) = self.inflight.lock() {
                inf.remove(&(cx, cz));
            }
        }
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
        if let Some(hit) = self.try_chunk(cx, cz) {
            return hit;
        }
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

fn worker(
    seed: i64,
    rx: mpsc::Receiver<Request>,
    ready: Arc<Mutex<HashMap<(i32, i32), Arc<EncodedChunk>>>>,
    inflight: Arc<Mutex<HashSet<(i32, i32)>>>,
) {
    tracing::info!(seed, "worldgen worker starting (building NoiseRouter)");
    let started = std::time::Instant::now();
    let gen = neutron_worldgen::ChunkGenerator::new(seed);
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "worldgen worker ready"
    );

    let mut noise_cache = NoiseCache::new();
    let mut queued: VecDeque<(i32, i32)> = VecDeque::new();
    let mut queued_set: HashSet<(i32, i32)> = HashSet::new();
    let mut spawn: Option<(f64, f64, f64)> = None;

    loop {
        let req = if queued.is_empty() {
            match rx.recv() {
                Ok(r) => r,
                Err(_) => break,
            }
        } else {
            match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => {
                    if let Some((cx, cz)) = queued.pop_front() {
                        queued_set.remove(&(cx, cz));
                        ensure_encoded(cx, cz, &gen, &mut noise_cache, &ready, &inflight);
                    }
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        };

        match req {
            Request::Chunk { cx, cz, resp } => {
                if let Ok(mut inf) = inflight.lock() {
                    inf.insert((cx, cz));
                }
                let encoded = ensure_encoded(cx, cz, &gen, &mut noise_cache, &ready, &inflight);
                let _ = resp.send(encoded);
            }
            Request::Prefetch { cx, cz } => {
                if ready.lock().map(|g| g.contains_key(&(cx, cz))).unwrap_or(false) {
                    continue;
                }
                if queued_set.insert((cx, cz)) {
                    queued.push_back((cx, cz));
                }
            }
            Request::Spawn { resp } => {
                if spawn.is_none() {
                    let encoded = ensure_encoded(0, 0, &gen, &mut noise_cache, &ready, &inflight);
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

fn ensure_encoded(
    cx: i32,
    cz: i32,
    gen: &neutron_worldgen::ChunkGenerator,
    noise_cache: &mut NoiseCache,
    ready: &Arc<Mutex<HashMap<(i32, i32), Arc<EncodedChunk>>>>,
    inflight: &Arc<Mutex<HashSet<(i32, i32)>>>,
) -> Arc<EncodedChunk> {
    if let Ok(guard) = ready.lock() {
        if let Some(hit) = guard.get(&(cx, cz)) {
            return hit.clone();
        }
    }

    let t0 = std::time::Instant::now();
    let chunk = gen.generate_chunk_cached(cx, cz, noise_cache);
    if noise_cache.len() > NOISE_CACHE_CAP {
        // Drop an arbitrary older column; HashMap iteration order is fine
        // here (cache is a speed hint, not a correctness store).
        if let Some(key) = noise_cache.keys().next().copied() {
            noise_cache.remove(&key);
        }
    }
    let body = chunk_sender::encode_playable_chunk(&chunk);
    let ms = t0.elapsed().as_millis() as u64;
    tracing::debug!(cx, cz, ms, bytes = body.len(), "generated chunk");
    let encoded = Arc::new(EncodedChunk {
        body,
        heightmap: chunk.heightmap,
    });
    if let Ok(mut guard) = ready.lock() {
        if guard.len() >= READY_CACHE_CAP {
            if let Some(key) = guard.keys().next().copied() {
                guard.remove(&key);
            }
        }
        guard.insert((cx, cz), encoded.clone());
    }
    if let Ok(mut inf) = inflight.lock() {
        inf.remove(&(cx, cz));
    }
    encoded
}
