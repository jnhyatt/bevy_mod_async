use bevy_asset::{Asset, AssetPath, AssetServer, Handle};
use bevy_ecs::{bundle::Bundle, entity::Entity};

use crate::{TaskContext, WithWorld};

pub trait CommonUsesTaskExt {
    fn spawn(&self, bundle: impl Bundle) -> WithWorld<Entity>;
    fn despawn(&self, e: Entity) -> WithWorld<bool>;
    fn load_asset<'a, A: Asset>(
        &self,
        path: impl Into<AssetPath<'a>> + Send + 'static,
    ) -> WithWorld<Handle<A>>;
}

impl CommonUsesTaskExt for TaskContext {
    fn spawn(&self, bundle: impl Bundle) -> WithWorld<Entity> {
        self.with_world(|world| world.spawn(bundle).id())
    }

    fn despawn(&self, e: Entity) -> WithWorld<bool> {
        self.with_world(move |world| world.despawn(e))
    }

    fn load_asset<'a, A: Asset>(
        &self,
        path: impl Into<AssetPath<'a>> + Send + 'static,
    ) -> WithWorld<Handle<A>> {
        self.with_world(|world| world.resource::<AssetServer>().load(path))
    }
}
