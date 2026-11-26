use std::sync::Arc;

use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode, window::Window};

use crate::assets::AssetManager;
use crate::renderer::Renderer;
use crate::scene::Scene;

pub struct Engine {
    pub renderer: Renderer,
    pub asset_manager: AssetManager,

    pub scene: Option<Scene>,

    pub window: Arc<Window>,
}

impl Engine {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let asset_manager = AssetManager::new();

        let test_scene: Arc<crate::assets::SceneTemplate> =
            asset_manager.get_or_load("test_scene.ron").await?;

        Ok(Self {
            renderer: Renderer::new(window.clone()).await?,
            asset_manager,
            scene: None,
            window,
        })
    }

    pub fn handle_key(&self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {}
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height)
    }

    pub fn update(&mut self) -> anyhow::Result<()> {
        if self.scene.is_none() {
            if let Some(scene_template) = self.asset_manager.try_load("test_scene.ron") {
                self.scene = Some(Scene::from_template(self, &scene_template)?);
            }
        }

        Ok(())
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        self.update()?; // TODO: call this more sensibly
        self.window.request_redraw();

        if let Some(scene) = &self.scene {
            self.renderer.update_gameobjects(&scene.game_objects);
        }

        self.renderer.render()?;
        Ok(())
    }
}
