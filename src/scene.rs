use crate::engine::Engine;
use crate::game_object::GameObject;

pub struct Scene {
    pub game_objects: Vec<GameObject>,
}

impl Scene {
    pub fn from_template(
        engine: &mut Engine,
        template: &crate::assets::SceneTemplate,
    ) -> anyhow::Result<Self> {
        Ok(Self {
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
