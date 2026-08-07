//! Shared types for the benchmark harness.

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ServerType {
    Vanilla,
    Paper,
    Folia,
    Pumpkin,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Size {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Scenario {
    JoinStorm,
    Distributed,
    Movement,
    Spread,
    ChunkGen,
}

impl Size {
    pub fn bot_count(self) -> usize {
        match self {
            Size::Small => 10,
            Size::Medium => 100,
            Size::Large => 1000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Size::Small => "small",
            Size::Medium => "medium",
            Size::Large => "large",
        }
    }
}

impl Scenario {
    pub fn label(self) -> &'static str {
        match self {
            Scenario::JoinStorm => "join-storm",
            Scenario::Distributed => "distributed",
            Scenario::Movement => "movement",
            Scenario::Spread => "spread",
            Scenario::ChunkGen => "chunk-gen",
        }
    }

    pub fn all() -> &'static [Scenario] {
        &[
            Scenario::JoinStorm,
            Scenario::Distributed,
            Scenario::Movement,
            Scenario::Spread,
            Scenario::ChunkGen,
        ]
    }
}

impl ServerType {
    pub fn label(self) -> &'static str {
        match self {
            ServerType::Vanilla => "vanilla",
            ServerType::Paper => "paper",
            ServerType::Folia => "folia",
            ServerType::Pumpkin => "pumpkin",
        }
    }

    pub fn is_java(self) -> bool {
        matches!(self, ServerType::Vanilla | ServerType::Paper | ServerType::Folia)
    }
}
