//! Skins: the pilot's osu! stable skins and mania scroll speed for the Label playfield, read in
//! place from `Skins/` and the user cfg (ADR 0019).

pub mod dto;
mod params;
mod service;

pub use params::SkinParams;
pub use service::SkinsService;
