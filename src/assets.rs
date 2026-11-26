use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::anyhow;
use aseprite::AsepriteFile;
use futures::{future::try_join_all, lock::Mutex};
use serde::{Deserialize, Serialize};

use crate::transform::Transform;

// the future must be Send on native and !Send on web
#[cfg(target_arch = "wasm32")]
pub trait FromAssets: Sized + Send + Sync {
    fn from_assets(
        asset_manager: &AssetManagerInner,
        filename: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Self>>;
}

#[cfg(not(target_arch = "wasm32"))]
pub trait FromAssets: Sized + Send + Sync {
    fn from_assets(
        asset_manager: &AssetManagerInner,
        filename: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Self>> + std::marker::Send;
}

#[derive(Serialize, Deserialize)]
struct SerGameObjectTemplate {
    size: (u32, u32),
    sprite_filename: String,
}

pub struct GameObjectTemplate {
    pub size: (u32, u32),
    pub sprite_filename: String,
    pub sprite: Arc<AsepriteFile>,
}

impl FromAssets for GameObjectTemplate {
    async fn from_assets(
        asset_manager: &AssetManagerInner,
        filename: &str,
    ) -> anyhow::Result<Self> {
        let bytes = load_asset(filename).await?;
        let ser_self: SerGameObjectTemplate = ron::from_str(str::from_utf8(&bytes)?)?;

        let sprite = asset_manager
            .get_or_load::<aseprite::AsepriteFile>(&ser_self.sprite_filename)
            .await?;

        Ok(Self {
            size: ser_self.size,
            sprite_filename: ser_self.sprite_filename,
            sprite,
        })
    }
}

impl FromAssets for aseprite::AsepriteFile {
    async fn from_assets(
        _asset_manager: &AssetManagerInner,
        filename: &str,
    ) -> anyhow::Result<Self> {
        let bytes = load_asset(filename).await?;
        Ok(AsepriteFile::from_reader(&bytes[..])?)
    }
}

#[derive(Serialize, Deserialize)]
struct SerSceneGameObjectInstantiation {
    template_filename: String,
    transform: Transform,
}

#[derive(Serialize, Deserialize)]
struct SerSceneTemplate {
    game_objects: Vec<SerSceneGameObjectInstantiation>,
}

pub struct SceneGameObjectInstantiation {
    pub template: Arc<GameObjectTemplate>,
    pub transform: Transform,
}

pub struct SceneTemplate {
    pub game_objects: Vec<SceneGameObjectInstantiation>,
}

impl FromAssets for SceneTemplate {
    async fn from_assets(
        asset_manager: &AssetManagerInner,
        filename: &str,
    ) -> anyhow::Result<Self> {
        let bytes = load_asset(filename).await?;
        let ser_self: SerSceneTemplate = ron::from_str(str::from_utf8(&bytes)?)?;

        let game_objects = try_join_all(ser_self.game_objects.iter().map(async |ser_go| {
            let template: Arc<GameObjectTemplate> =
                asset_manager.get_or_load(&ser_go.template_filename).await?;

            Ok::<_, anyhow::Error>(SceneGameObjectInstantiation {
                template,
                transform: ser_go.transform,
            })
        }))
        .await?;

        Ok(Self { game_objects })
    }
}

pub struct AssetManagerInner {
    assets: Mutex<HashMap<(TypeId, String), Arc<dyn Any + Send + Sync>>>,
    active_loads: Mutex<HashSet<(TypeId, String)>>,
}

impl AssetManagerInner {
    pub fn new() -> Self {
        Self {
            assets: Mutex::new(HashMap::new()),
            active_loads: Mutex::new(HashSet::new()),
        }
    }

    pub async fn get_or_load<T: 'static + FromAssets>(
        &self,
        filename: &str,
    ) -> anyhow::Result<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let key = (type_id, filename.to_string());

        {
            let cache = self.assets.lock().await;
            if let Some(gen_asset) = cache.get(&key) {
                match gen_asset.clone().downcast::<T>() {
                    Ok(asset) => {
                        return Ok(asset);
                    }
                    Err(_) => unreachable!("item insert as (TypeId::of::<T>(), ...) is of type T"),
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

    pub fn try_load<T: 'static + FromAssets>(self: &Arc<Self>, filename: &str) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let key = (type_id, filename.to_string());

        if let Some(cache) = self.assets.try_lock() {
            if let Some(gen_asset) = cache.get(&key) {
                match gen_asset.clone().downcast::<T>() {
                    Ok(asset) => {
                        return Some(asset);
                    }
                    Err(_) => unreachable!("item insert as (TypeId::of::<T>(), ...) is of type T"),
                }
            }
        }

        // if we are already loading the asset we must wait
        if let Some(_) = self.active_loads.try_lock()?.get(&key) {
            return None;
        }

        let this = self.clone();
        let move_filename = filename.to_owned();
        crate::future::spawn_task(async move {
            if let Err(e) = this.get_or_load::<T>(&move_filename).await {
                log::error!("error while loading asset: {}", e);
            }
        });

        None
    }
}

#[derive(Clone)]
pub struct AssetManager {
    inner: Arc<AssetManagerInner>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AssetManagerInner::new()),
        }
    }

    pub async fn get_or_load<T: 'static + FromAssets>(
        &self,
        filename: &str,
    ) -> anyhow::Result<Arc<T>> {
        self.inner.get_or_load(filename).await
    }

    pub fn try_load<T: 'static + FromAssets>(&self, filename: &str) -> Option<Arc<T>> {
        self.inner.try_load(filename)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn load_asset(filename: &str) -> anyhow::Result<Vec<u8>> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow!("Executable has no parent directory"))?;

    let asset_path = exe_dir.join("assets").join(filename);

    Ok(std::fs::read(&asset_path)?)
}

#[cfg(target_arch = "wasm32")]
pub async fn load_asset(filename: &str) -> anyhow::Result<Vec<u8>> {
    use gloo_net::http::Request;

    let url = format!("/assets/{}", filename.trim_start_matches("/"));

    let response = Request::get(&url).send().await?;

    if !response.ok() {
        return Err(anyhow!(
            "HTTP Error {} while fetching {}",
            response.status(),
            url
        ));
    }

    Ok(response.binary().await?)
}
