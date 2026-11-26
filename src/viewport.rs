use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable, Serialize, Deserialize)]
pub struct Viewport {
    pub center: Vec2,
    pub width: f32,
}
