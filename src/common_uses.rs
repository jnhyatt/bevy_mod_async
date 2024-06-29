use bevy_ecs::{bundle::Bundle, entity::Entity};

use crate::{TaskContext, WithWorld};

#[cfg(feature = "asset")]
use {
    crate::async_asset::{AssetLoadError, AsyncAssetTaskExt},
    bevy_asset::{Asset, AssetPath, AssetServer, Handle, RecursiveDependencyLoadState},
    futures::StreamExt,
    std::future::Future,
};

pub trait CommonUsesTaskExt {
    fn spawn(&self, bundle: impl Bundle) -> WithWorld<Entity>;
    fn despawn(&self, e: Entity) -> WithWorld<bool>;

    #[cfg(feature = "asset")]
    fn load_asset<'a, A: Asset>(
        &self,
        path: impl Into<AssetPath<'a>> + Send + 'static,
    ) -> impl Future<Output = Result<Handle<A>, AssetLoadError>>;
}

impl CommonUsesTaskExt for TaskContext {
    fn spawn(&self, bundle: impl Bundle) -> WithWorld<Entity> {
        self.with_world(|world| world.spawn(bundle).id())
    }

    fn despawn(&self, e: Entity) -> WithWorld<bool> {
        self.with_world(move |world| world.despawn(e))
    }

    #[cfg(feature = "asset")]
    fn load_asset<'a, A: Asset>(
        &self,
        path: impl Into<AssetPath<'a>> + Send + 'static,
    ) -> impl Future<Output = Result<Handle<A>, AssetLoadError>> {
        async {
            let handle = self
                .with_world(|world| world.resource::<AssetServer>().load(path))
                .await;
            let mut states = self.get_load_state(handle.clone());
            while let Some(x) = states.next().await {
                match x {
                    RecursiveDependencyLoadState::NotLoaded => return Err(AssetLoadError),
                    RecursiveDependencyLoadState::Loading => {}
                    RecursiveDependencyLoadState::Loaded => return Ok(handle),
                    RecursiveDependencyLoadState::Failed => return Err(AssetLoadError),
                }
            }
            Err(AssetLoadError)
        }
    }
}
