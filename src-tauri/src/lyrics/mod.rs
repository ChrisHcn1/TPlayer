//! 歌词模块：LRC 解析、偏移持久化、在线抓取

pub mod online;
pub mod parser;

pub use parser::Lyrics;
