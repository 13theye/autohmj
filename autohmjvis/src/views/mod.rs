// src/views/mod.rs
pub mod animation;
pub use animation::{AnimationController, AnimationEvent};

pub mod anim_hangeul;

pub mod background;
pub use background::BackgroundManager;

pub mod grid_cell;
pub use grid_cell::{GridCell, GridCellChar};

pub mod grid;
pub use grid::{TextGrid, TextGridPosition};
