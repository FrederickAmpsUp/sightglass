// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Copy, Clone, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Transform {
    pub position: glam::Vec2,
    pub rotation: f32,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: glam::Vec2::ZERO,
        rotation: 0.0,
    };
}
