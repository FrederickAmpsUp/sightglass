// SPDX-FileCopyrightText: 2026 Frederick Ziola
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;

use aseprite::AsepriteFile;
use etagere::size2;
use glam::Vec2;

#[derive(Clone)]
pub struct SpriteId(pub u32);

pub struct SpriteRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

pub struct SpriteAtlas {
    atlas_type: wgpu::Texture,
    atlas_albd: wgpu::Texture,
    atlas_norm: wgpu::Texture,

    filename_ids: HashMap<String, SpriteId>,
    regions: Vec<SpriteRegion>,
    allocator: etagere::BucketedAtlasAllocator,
}

impl SpriteAtlas {
    const PADDING: u16 = 1;

    pub fn new(
        atlas_type: wgpu::Texture,
        atlas_albd: wgpu::Texture,
        atlas_norm: wgpu::Texture,
    ) -> Self {
        Self {
            atlas_type,
            atlas_albd,
            atlas_norm,
            filename_ids: HashMap::new(),
            regions: vec![],
            allocator: etagere::BucketedAtlasAllocator::new(size2(1024, 1024)),
        }
    }

    pub fn sprite(
        &mut self,
        filename: Option<&str>,
        sprite: &AsepriteFile,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<SpriteId> {
        if let Some(filename) = filename {
            if let Some(id) = self.filename_ids.get(&filename.to_string()) {
                return Ok(id.clone());
            }
        }

        let alloc = self
            .allocator
            .allocate(etagere::size2(
                (sprite.width() + Self::PADDING * 2) as i32,
                (sprite.height() + Self::PADDING * 2) as i32,
            ))
            .ok_or(anyhow::anyhow!("failed to allocate sprite"))?;

        let x = alloc.rectangle.min.x as u32 + Self::PADDING as u32;
        let y = alloc.rectangle.min.y as u32 + Self::PADDING as u32;

        let write_layer = |texture: &wgpu::Texture, data: &[u8], bytes_per_px: u32| {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_px * sprite.width() as u32),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: sprite.width() as u32,
                    height: sprite.height() as u32,
                    depth_or_array_layers: 1,
                },
            );
        };

        let get_pixels = |layer_id, frame_id| -> anyhow::Result<&aseprite::Pixels> {
            let cel = sprite
                .resolve_cel(
                    sprite
                        .layer_ref(layer_id)
                        .ok_or(anyhow::anyhow!("failed to create layer ref"))?,
                    frame_id,
                )
                .ok_or(anyhow::anyhow!("failed to resolve cel"))?;

            match &cel.kind {
                aseprite::CelKind::Raw { pixels, x: _, y: _ }
                | aseprite::CelKind::Compressed {
                    pixels,
                    x: _,
                    y: _,
                    original_compressed: _,
                } => Ok(&pixels),
                _ => Err(anyhow::anyhow!("bad cel kind")),
            }
        };

        for (layer_index, layer) in sprite.layers().iter().enumerate() {
            match layer.name.as_str() {
                "type" => {
                    let data: Vec<u8> = get_pixels(layer_index, 0)?
                        .data
                        .iter()
                        .step_by(4)
                        .copied()
                        .collect(); // we love iterators (this gets every 4th byte)

                    write_layer(&self.atlas_type, &data, 1);
                }
                "albd" => {
                    write_layer(&self.atlas_albd, &get_pixels(layer_index, 0)?.data, 4);
                }
                "norm" => {
                    write_layer(&self.atlas_norm, &get_pixels(layer_index, 0)?.data, 4);
                }

                name => {
                    log::warn!("unexpected layer {}", name);
                }
            }
        }

        let id = SpriteId(self.regions.len() as u32);
        self.regions.push(SpriteRegion {
            x,
            y,
            width: sprite.width() as u32,
            height: sprite.height() as u32,
        });

        if let Some(filename) = filename {
            self.filename_ids.insert(filename.to_string(), id.clone());
        }

        Ok(id)
    }

    pub fn get_offset(&self, sprite_id: SpriteId) -> Option<(u32, u32)> {
        let region = self.regions.get(sprite_id.0 as usize)?;
        Some((region.x, region.y))
    }
}
