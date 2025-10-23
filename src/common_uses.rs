use bevy_ecs::{bundle::Bundle, entity::Entity, message::Message};

use crate::{TaskContext, WithWorld};

#[cfg(feature = "asset")]
use {
    crate::async_asset::AsyncAssetTaskExt,
    bevy_asset::{
        Asset, AssetLoadError, AssetPath, AssetServer, Handle, RecursiveDependencyLoadState,
    },
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
    ) -> impl Future<Output = Result<Handle<A>, AssetLoadError>> + Send;

    fn write_message<M: Message>(&self, event: M) -> WithWorld<()>;
}

impl CommonUsesTaskExt for TaskContext {
    fn spawn(&self, bundle: impl Bundle) -> WithWorld<Entity> {
        self.with_world(|world| world.spawn(bundle).id())
    }

    fn despawn(&self, e: Entity) -> WithWorld<bool> {
        self.with_world(move |world| world.despawn(e))
    }

    #[cfg(feature = "asset")]
    async fn load_asset<'a, A: Asset>(
        &self,
        path: impl Into<AssetPath<'a>> + Send + 'static,
    ) -> Result<Handle<A>, AssetLoadError> {
        let handle = self
            .with_world(|world| world.resource::<AssetServer>().load(path))
            .await;
        let mut states = self.get_load_state(handle.id());
        while let Some(x) = states.next().await {
            match x {
                RecursiveDependencyLoadState::NotLoaded => {
                    return Err(AssetLoadError::AssetMetaReadError)
                } //TODO work out correct error to pass
                RecursiveDependencyLoadState::Loading => {}
                RecursiveDependencyLoadState::Loaded => return Ok(handle),
                RecursiveDependencyLoadState::Failed(error) => return Err(error.as_ref().clone()),
            }
        }
        Err(AssetLoadError::AssetMetaReadError)
    }

    fn write_message<M: Message>(&self, message: M) -> WithWorld<()> {
        self.with_world(move |world| {
            world.write_message(message);
        })
    }
}
