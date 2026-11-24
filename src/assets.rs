use std::{any::{Any, TypeId}, collections::HashMap, sync::Arc};

use aseprite::AsepriteFile;
use futures::{lock::Mutex, future::try_join_all};
use serde::{Serialize, Deserialize};
use anyhow::anyhow;

pub trait FromAssets : Sized + Send + Sync {
    async fn from_assets(asset_manager: &AssetManager, filename: &str) -> anyhow::Result<Self>;
}

#[derive(Serialize, Deserialize)]
struct SerGameObjectTemplate {
    width: u32,
    height: u32,
    sprite_filename: String
}

pub struct GameObjectTemplate {
    pub width: u32,
    pub height: u32,
    pub sprite: AsepriteFile,
}

impl FromAssets for GameObjectTemplate {
    async fn from_assets(_asset_manager: &AssetManager, filename: &str) -> anyhow::Result<Self> {
        let bytes = load_asset(filename).await?;
        let ser_self: SerGameObjectTemplate = ron::from_str(str::from_utf8(&bytes)?)?;

        let sprite_bytes = load_asset(&ser_self.sprite_filename).await?;
        let sprite = AsepriteFile::from_reader(&sprite_bytes[..])?;

        Ok(Self {
            width: ser_self.width,
            height: ser_self.height,
            sprite
        })
    }
}

#[derive(Serialize, Deserialize)]
struct SerSceneGameObjectInstantiation {
    template_filename: String,
    x: u32,
    y: u32,
    // TODO: rotation
}

#[derive(Serialize, Deserialize)]
struct SerSceneTemplate {
    game_objects: Vec<SerSceneGameObjectInstantiation>,
}

pub struct SceneGameObjectInstantiation {
    pub template: Arc<GameObjectTemplate>,
    pub x: u32,
    pub y: u32
}

pub struct SceneTemplate {
    pub game_objects: Vec<SceneGameObjectInstantiation>
}

impl FromAssets for SceneTemplate {
    async fn from_assets(asset_manager: &AssetManager, filename: &str) -> anyhow::Result<Self> {
        let bytes = load_asset(filename).await?;
        let ser_self: SerSceneTemplate = ron::from_str(str::from_utf8(&bytes)?)?;

        let game_objects = try_join_all(ser_self.game_objects.iter().map(async |ser_go| {
            let template: Arc<GameObjectTemplate> = asset_manager.get_or_load(&ser_go.template_filename).await?;

            Ok::<_, anyhow::Error>(SceneGameObjectInstantiation {
                template,
                x: ser_go.x, y: ser_go.y
            })
        })).await?;

        Ok(Self {
            game_objects
        })
    }
}

pub struct AssetManager {
    assets: Mutex<HashMap<(TypeId, String), Arc<dyn Any + Send + Sync>>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            assets: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_load<T: 'static + FromAssets>(&self, filename: &str) -> anyhow::Result<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let key = (type_id, filename.to_string());

        {
            let cache = self.assets.lock().await;
            if let Some(gen_asset) = cache.get(&key) {
                match gen_asset.clone().downcast::<T>() {
                    Ok(asset) => { return Ok(asset); },
                    Err(_) => unreachable!("item insert as (TypeId::of::<T>(), ...) is of type T")
                }
            }
        }

        let asset = T::from_assets(self, filename).await?;
        let asset_arc = Arc::new(asset);
        let gen_asset_arc = Arc::clone(&asset_arc) as Arc<dyn Any + Send + Sync>;

        let mut cache = self.assets.lock().await;
        cache.insert(key, gen_asset_arc);

        Ok(asset_arc)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn load_asset(filename: &str) -> anyhow::Result<Vec<u8>> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent()
        .ok_or_else(|| anyhow!("Executable has no parent directory"))?;

    let asset_path = exe_dir.join("assets").join(filename);

    Ok(std::fs::read(&asset_path)?)
}

#[cfg(target_arch = "wasm32")]
pub async fn load_asset(filename: &str) -> anyhow::Result<Vec<u8>> {
    use gloo_net::http::Request;

    let url = format!("/assets/{}", filename.trim_start_matches("/"));

    let response = Request::get(&url)
        .send().await?;

    if !response.ok() {
        return Err(anyhow!("HTTP Error {} while fetching {}", response.status(), url));
    }

    Ok(response.binary().await?)
}
