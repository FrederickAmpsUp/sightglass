use std::sync::Arc;

use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode, window::Window};

use crate::assets::AssetManager;
use crate::renderer::Renderer;

pub struct Engine {
    pub renderer: Renderer,
    pub asset_manager: AssetManager,

    pub window: Arc<Window>,
}

impl Engine {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let asset_manager = AssetManager::new();

        Ok(Self {
            renderer: Renderer::new(window.clone()).await?,
            asset_manager,
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

    pub fn update(&mut self) {}

    pub fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        self.renderer.render()?;
        Ok(())
    }
}
