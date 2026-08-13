//! Server state and configuration.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, AtomicU64};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// ServerConfig
// ---------------------------------------------------------------------------

/// Configuration for the Neutron server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// TCP port to listen on.
    pub port: u16,
    /// World seed.
    pub seed: i64,
    /// Server MOTD (message of the day).
    pub motd: String,
    /// Maximum number of players.
    pub max_players: i32,
    /// View distance in chunks.
    pub view_distance: i32,
    /// Whether to enforce online-mode (Mojang authentication).
    pub online_mode: bool,
    /// Compression threshold for packets (-1 = disabled).
    pub compression_threshold: i32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 25565,
            seed: 0,
            motd: "A Neutron Server".to_string(),
            max_players: 20,
            view_distance: 10,
            online_mode: false,
            compression_threshold: 256,
        }
    }
}

// ---------------------------------------------------------------------------
// PlayerState
// ---------------------------------------------------------------------------

/// Per-player state tracked by the server.
#[derive(Debug)]
pub struct PlayerState {
    /// Protocol entity ID for this player.
    pub entity_id: i32,
    /// Player UUID.
    pub uuid: uuid::Uuid,
    /// Player username.
    pub username: String,
    /// Current X position.
    pub x: f64,
    /// Current Y position.
    pub y: f64,
    /// Current Z position.
    pub z: f64,
    /// Current chunk X.
    pub chunk_x: i32,
    /// Current chunk Z.
    pub chunk_z: i32,
    /// Chunks already sent to this player.
    pub sent_chunks: HashSet<(i32, i32)>,
    /// Last KeepAlive ID sent to this player (for timeout tracking).
    pub last_keepalive_id: Option<i64>,
    /// Tick when the last KeepAlive was sent.
    pub last_keepalive_tick: u64,
    /// Whether the player has responded to the last KeepAlive.
    pub keepalive_pending: bool,
    /// Game mode.
    pub game_mode: u8,
    /// Whether this player has completed the login sequence.
    pub is_playing: bool,
}

// ---------------------------------------------------------------------------
// ServerState
// ---------------------------------------------------------------------------

/// Shared server state, protected by async locks.
pub struct ServerState {
    /// Server configuration.
    pub config: ServerConfig,
    /// Currently connected players, keyed by UUID.
    pub players: RwLock<HashMap<uuid::Uuid, PlayerState>>,
    /// Global tick counter.
    pub tick_count: AtomicU64,
    /// Next entity ID to assign.
    next_entity_id: AtomicI32,
    /// Server start time.
    pub start_time: Instant,
}

impl ServerState {
    /// Create a new ServerState.
    pub fn new(config: ServerConfig) -> Self {
        let start_time = Instant::now();
        Self {
            config,
            players: RwLock::new(HashMap::new()),
            tick_count: AtomicU64::new(0),
            next_entity_id: AtomicI32::new(1),
            start_time,
        }
    }

    /// Allocate a new entity ID.
    pub fn next_entity_id(&self) -> i32 {
        self.next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the current tick count.
    pub fn current_tick(&self) -> u64 {
        self.tick_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Register a player. Returns the entity ID.
    pub async fn register_player(&self, uuid: uuid::Uuid, username: String) -> i32 {
        let entity_id = self.next_entity_id();
        let player = PlayerState {
            entity_id,
            uuid,
            username: username.clone(),
            x: 0.0,
            y: 65.0, // spawn above ground
            z: 0.0,
            chunk_x: 0,
            chunk_z: 0,
            sent_chunks: HashSet::new(),
            last_keepalive_id: None,
            last_keepalive_tick: 0,
            keepalive_pending: false,
            game_mode: 1, // creative
            is_playing: false,
        };
        self.players.write().await.insert(uuid, player);
        tracing::info!(
            uuid = %uuid,
            username = %username,
            entity_id,
            "player registered"
        );
        entity_id
    }

    /// Remove a player.
    pub async fn remove_player(&self, uuid: &uuid::Uuid) -> Option<PlayerState> {
        let player = self.players.write().await.remove(uuid);
        if let Some(ref p) = player {
            tracing::info!(
                uuid = %p.uuid,
                username = %p.username,
                "player disconnected"
            );
        }
        player
    }

    /// Get the number of connected players.
    pub async fn player_count(&self) -> usize {
        self.players.read().await.len()
    }

    /// Update a player's position.
    pub async fn update_player_position(&self, uuid: &uuid::Uuid, x: f64, y: f64, z: f64) {
        if let Some(player) = self.players.write().await.get_mut(uuid) {
            player.x = x;
            player.y = y;
            player.z = z;
            player.chunk_x = (x.floor() as i32) >> 4;
            player.chunk_z = (z.floor() as i32) >> 4;
        }
    }

    /// Update a player's rotation.
    pub async fn update_player_rotation(&self, uuid: &uuid::Uuid, _yaw: f32, _pitch: f32) {
        // Rotation is tracked but not stored separately for now.
        let _ = (uuid, _yaw, _pitch);
    }

    /// Get all player UUIDs.
    pub async fn player_uuids(&self) -> Vec<uuid::Uuid> {
        self.players.read().await.keys().copied().collect()
    }

    /// Get player info (username, entity_id) for chat broadcast.
    pub async fn get_player_info(&self, uuid: &uuid::Uuid) -> Option<(String, i32)> {
        self.players
            .read()
            .await
            .get(uuid)
            .map(|p| (p.username.clone(), p.entity_id))
    }
}

// Shared reference to server state.
pub type SharedServer = Arc<ServerState>;
