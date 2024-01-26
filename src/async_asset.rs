use std::{
    collections::HashMap,
    fmt::Debug,
    pin::Pin,
    task::{Context, Poll},
};

use bevy_asset::{AssetServer, RecursiveDependencyLoadState, UntypedAssetId};
use bevy_ecs::system::{Res, ResMut, Resource};
use futures::{FutureExt, Stream, StreamExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use crate::{TaskContext, WithWorld};

pub trait AsyncAssetTaskExt {
    fn get_load_state(
        &self,
        id: impl Into<UntypedAssetId> + Send + 'static,
    ) -> impl Stream<Item = RecursiveDependencyLoadState>;
}

impl AsyncAssetTaskExt for TaskContext {
    fn get_load_state(
        &self,
        id: impl Into<UntypedAssetId> + Send + 'static,
    ) -> impl Stream<Item = RecursiveDependencyLoadState> {
        LoadStateStream::new(self.clone(), id.into())
    }
}

/// Notifies interested parties of changes to asset load states. Iterates asset handles
/// registered in [`AssetSubscriptions`] and checks their load state, emitting an update to
/// all subscribers if the state has changed.
pub fn notify_asset_events(
    mut subscriptions: ResMut<AssetSubscriptions>,
    assets: Res<AssetServer>,
) {
    let mut to_unsubscribe = Vec::new();
    for (id, tx) in &subscriptions.handles {
        let current = assets.recursive_dependency_load_state(*id);
        if current != *tx.borrow() {
            if tx.send(current).is_err() {
                // Channel is closed, unsubscribe this asset
                to_unsubscribe.push(*id);
            }
        }
    }
    for id in to_unsubscribe {
        subscriptions.handles.remove(&id);
    }
}

/// Manages interest in assets. Maintains a [`tokio::sync::watch::Sender`] for each asset
/// handle a client has expressed interest in. [`AssetSubscriptions::subscribe_to`] is used
/// to express interest in the load state for a given asset.
#[derive(Resource)]
pub struct AssetSubscriptions {
    handles: HashMap<UntypedAssetId, watch::Sender<RecursiveDependencyLoadState>>,
}

impl AssetSubscriptions {
    /// Subscribe to all asset load events for an asset. The resulting channel will
    /// immediately yield the current load state for the given asset, and subsequent changes
    /// to the load state will generate additional change events.
    pub fn subscribe_to(
        &mut self,
        id: UntypedAssetId,
        init: RecursiveDependencyLoadState,
    ) -> watch::Receiver<RecursiveDependencyLoadState> {
        let (tx, rx) = watch::channel(init);
        self.handles.insert(id, tx);
        rx
    }
}

impl Default for AssetSubscriptions {
    fn default() -> Self {
        Self {
            handles: Default::default(),
        }
    }
}

enum LoadStateStreamState {
    AwaitingWorld(WithWorld<watch::Receiver<RecursiveDependencyLoadState>>),
    HasStream(WatchStream<RecursiveDependencyLoadState>),
}

pub struct LoadStateStream {
    state: LoadStateStreamState,
}

impl LoadStateStream {
    pub fn new(cx: TaskContext, id: UntypedAssetId) -> Self {
        let fut = cx.with_world(move |world| {
            let assets = world.resource::<AssetServer>();
            let init = assets.recursive_dependency_load_state(id);
            world
                .resource_mut::<AssetSubscriptions>()
                .subscribe_to(id, init)
        });
        Self {
            state: LoadStateStreamState::AwaitingWorld(fut),
        }
    }
}

impl Stream for LoadStateStream {
    type Item = RecursiveDependencyLoadState;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.state {
            LoadStateStreamState::AwaitingWorld(fut) => match fut.poll_unpin(cx) {
                Poll::Ready(rx) => {
                    self.state = LoadStateStreamState::HasStream(WatchStream::new(rx));
                    self.poll_next(cx)
                }
                Poll::Pending => Poll::Pending,
            },
            LoadStateStreamState::HasStream(rx) => rx.poll_next_unpin(cx),
        }
    }
}

/// This is used to report a generic asset load error until Bevy 0.13 gives me a nicer API
/// to report exactly what went wrong.
#[derive(Debug)]
pub struct AssetLoadError;
