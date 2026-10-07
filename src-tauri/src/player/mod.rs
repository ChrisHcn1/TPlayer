//! 播放模块：引擎、状态机、事件

pub mod engine;
pub mod handle;
pub mod persist;
pub mod state;

pub use engine::{spawn_engine, EngineCapabilities, EngineHandle};
pub use handle::{spawn_progress_clock, PlayerHandle};
pub use persist::PersistedPlayback;
pub use state::{
    PlayContext, PlayContextSource, PlayMode, PlaybackEvent, PlaybackSnapshot, PlaybackStatus,
};
