use std::sync::Arc;

use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode, window::Window};

use crate::renderer::Renderer;

pub struct Engine {
    pub renderer: Renderer,

    pub window: Arc<Window>,
}

impl Engine {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        Ok(Self {
            renderer: Renderer::new(window.clone()).await?,
            window,
        })
    }

    pub fn handle_key(&self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {}
        }
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        self.renderer.render()
    }
}
