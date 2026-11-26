// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::engine::Engine;
use crate::transform::Transform;

pub struct GameObject {
    pub transform: Transform,
    pub size: (u32, u32),
    pub sprite_id: crate::sprite_atlas::SpriteId,
}

impl GameObject {
    pub fn from_template(
        engine: &mut Engine,
        template: &crate::assets::GameObjectTemplate,
    ) -> anyhow::Result<Self> {
        let sprite_id = engine.renderer.sprite_atlas.sprite(
            Some(&template.sprite_filename),
            &template.sprite,
            &engine.renderer.queue,
        )?;

        Ok(Self {
            transform: Transform::IDENTITY,
            size: template.size,
            sprite_id,
        })
    }

    pub fn with_transform(mut self, transform: &Transform) -> Self {
        self.transform = transform.clone();
        self
    }
}
