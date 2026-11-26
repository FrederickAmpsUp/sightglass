// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

use glam::Vec2;

use crate::engine::Engine;
use crate::game_object::GameObject;
use crate::viewport::Viewport;

pub struct Scene {
    pub viewport: Viewport,

    pub game_objects: Vec<GameObject>,
}

impl Scene {
    pub fn from_template(
        engine: &mut Engine,
        template: &crate::assets::SceneTemplate,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            viewport: template.initial_viewport,
            game_objects: template
                .game_objects
                .iter()
                .map(|object_template| {
                    Ok::<_, anyhow::Error>(
                        GameObject::from_template(engine, &object_template.template)?
                            .with_transform(&object_template.transform),
                    )
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
