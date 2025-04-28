use crate::{TaskContext, WithWorld};
use bevy_ecs::{
    bundle::{Bundle, BundleFromComponents},
    entity::Entity,
    world::{error::EntityMutableFetchError, EntityWorldMut},
};

#[derive(Clone)]
pub struct AsyncEntity {
    pub entity: Entity,
    pub task_context: TaskContext,
}

impl AsyncEntity {
    /// Adds a [`Bundle`] of components to the entity.
    ///
    /// This will overwrite any previous value(s) of the same component type.
    pub fn insert(&self, bundle: impl Bundle) -> WithWorld<()> {
        let e = self.entity;
        self.task_context.with_world(move |world| {
            world.entity_mut(e).insert(bundle);
        })
    }

    /// Despawns the given `entity`, if it exists. This will also remove all of the entity's
    /// [`Component`]s. Returns `true` if the `entity` is successfully despawned and `false` if
    /// the `entity` does not exist.
    pub fn despawn(&self) -> WithWorld<bool> {
        let e = self.entity;
        self.task_context.with_world(move |world| world.despawn(e))
    }

    /// Removes a [`Bundle`] of components from the entity.
    pub fn remove<T: Bundle>(&self) -> WithWorld<()> {
        let e = self.entity;
        self.task_context.with_world(move |world| {
            world.entity_mut(e).remove::<T>();
        })
    }

    /// Removes all components associated with the entity.
    pub fn clear(&self) -> WithWorld<()> {
        let e = self.entity;
        self.task_context.with_world(move |world| {
            world.entity_mut(e).clear();
        })
    }

    /// Removes all components in the [`Bundle`] from the entity and returns their previous values.
    ///
    /// **Note:** If the entity does not have every component in the bundle, this method will not
    /// remove any of them.
    pub fn take<T: Bundle + BundleFromComponents>(&self) -> WithWorld<Option<T>> {
        let e = self.entity;
        self.task_context
            .with_world(move |world| world.entity_mut(e).take::<T>())
    }
}

pub trait AsyncEntityTaskExt {
    /// Returns an [`AsyncEntity`], which is a thin wrapper over [`TaskContext`] that allows async
    /// operations on an entity:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_mod_async::prelude::*;
    /// # use std::time::Duration;
    /// # #[derive(Component)]
    /// # struct Player;
    /// # #[derive(Component)]
    /// # struct Dead;
    /// # App::new()
    /// #     .add_plugins((MinimalPlugins, AssetPlugin::default(), AsyncTasksPlugin))
    /// #     .add_systems(Startup, |world: &mut World| {
    /// #         world.spawn_task(|cx| async move {
    /// let entity = cx.spawn(Player).await;
    /// cx.entity(entity).insert(Dead).await;
    /// cx.sleep(Duration::from_secs(1)).await;
    /// cx.entity(entity).despawn().await;
    /// #         cx.send_event(AppExit::Success).await;
    /// #     });
    /// # }).run();
    /// ```
    ///
    /// Note that, since all methodss on [`AsyncEntity`] return a [`WithWorld`], there will be a
    /// one-frame delay between tasks that are `.awwait`ed (rather than `.detach()`ed).
    fn entity(&self, entity: Entity) -> AsyncEntity;

    /// Execute a task with exclusive, mutable access so the given entity. This is a thin wrapper
    /// around [`TaskContext::with_world`] that provides an [`EntityWorldMut`] for the given entity.
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_mod_async::prelude::*;
    /// # #[derive(Component)]
    /// # struct Player;
    /// # #[derive(Component)]
    /// # struct Controls;
    /// # #[derive(Component)]
    /// # struct Dead;
    /// # App::new()
    /// #     .add_plugins((MinimalPlugins, AssetPlugin::default(), AsyncTasksPlugin))
    /// #     .add_systems(Startup, |world: &mut World| {
    /// #         world.spawn_task(|cx| async move {
    /// let entity = cx.spawn((Player, Controls)).await;
    /// cx.with_entity(entity, |mut player| {
    ///     player.insert(Dead).remove::<Controls>();
    /// }).await;
    /// #         });
    /// #         world.send_event(AppExit::Success);
    /// #     })
    /// #     .run();
    /// ```
    fn with_entity<R, F>(
        &self,
        entity: Entity,
        f: F,
    ) -> WithWorld<Result<R, EntityMutableFetchError>>
    where
        R: Send + 'static,
        F: FnOnce(EntityWorldMut) -> R + Send + 'static;
}

impl AsyncEntityTaskExt for TaskContext {
    fn entity(&self, entity: Entity) -> AsyncEntity {
        AsyncEntity {
            entity,
            task_context: self.clone(),
        }
    }

    fn with_entity<R, F>(
        &self,
        entity: Entity,
        f: F,
    ) -> WithWorld<Result<R, EntityMutableFetchError>>
    where
        R: Send + 'static,
        F: FnOnce(EntityWorldMut) -> R + Send + 'static,
    {
        self.with_world(move |world| Ok(f(world.get_entity_mut(entity)?)))
    }
}
