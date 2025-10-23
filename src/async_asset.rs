use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use bevy_asset::{AssetLoadError, AssetServer, RecursiveDependencyLoadState, UntypedAssetId};
use bevy_ecs::{
    resource::Resource,
    system::{Res, ResMut},
};
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

/// Because we can't implement [PartialEq] on a foreign type, create our own trait that mirrors the interface
trait PartialEquality {
    fn eq(&self, other: &Self) -> bool;
}

impl PartialEquality for AssetLoadError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::RequestedHandleTypeMismatch {
                    path: l_path,
                    requested: l_requested,
                    actual_asset_name: l_actual_asset_name,
                    loader_name: l_loader_name,
                },
                Self::RequestedHandleTypeMismatch {
                    path: r_path,
                    requested: r_requested,
                    actual_asset_name: r_actual_asset_name,
                    loader_name: r_loader_name,
                },
            ) => {
                l_path == r_path
                    && l_requested == r_requested
                    && l_actual_asset_name == r_actual_asset_name
                    && l_loader_name == r_loader_name
            }
            (
                Self::MissingAssetLoader {
                    loader_name: l_loader_name,
                    asset_type_id: l_asset_type_id,
                    extension: l_extension,
                    asset_path: l_asset_path,
                },
                Self::MissingAssetLoader {
                    loader_name: r_loader_name,
                    asset_type_id: r_asset_type_id,
                    extension: r_extension,
                    asset_path: r_asset_path,
                },
            ) => {
                l_loader_name == r_loader_name
                    && l_asset_type_id == r_asset_type_id
                    && l_extension == r_extension
                    && l_asset_path == r_asset_path
            }
            (
                Self::MissingAssetLoaderForExtension(l0),
                Self::MissingAssetLoaderForExtension(r0),
            ) => l0 == r0,
            (Self::MissingAssetLoaderForTypeName(l0), Self::MissingAssetLoaderForTypeName(r0)) => {
                l0 == r0
            }
            (
                Self::MissingAssetLoaderForTypeIdError(l0),
                Self::MissingAssetLoaderForTypeIdError(r0),
            ) => l0 == r0,
            (Self::AssetReaderError(l0), Self::AssetReaderError(r0)) => l0 == r0,
            (Self::MissingAssetSourceError(l0), Self::MissingAssetSourceError(r0)) => l0 == r0,
            (
                Self::MissingProcessedAssetReaderError(l0),
                Self::MissingProcessedAssetReaderError(r0),
            ) => l0 == r0,
            (
                Self::DeserializeMeta {
                    path: l_path,
                    error: l_error,
                },
                Self::DeserializeMeta {
                    path: r_path,
                    error: r_error,
                },
            ) => l_path == r_path && l_error == r_error,
            (
                Self::CannotLoadProcessedAsset { path: l_path },
                Self::CannotLoadProcessedAsset { path: r_path },
            ) => l_path == r_path,
            (
                Self::CannotLoadIgnoredAsset { path: l_path },
                Self::CannotLoadIgnoredAsset { path: r_path },
            ) => l_path == r_path,
            (
                Self::AssetLoaderPanic {
                    path: l_path,
                    loader_name: l_loader_name,
                },
                Self::AssetLoaderPanic {
                    path: r_path,
                    loader_name: r_loader_name,
                },
            ) => l_path == r_path && l_loader_name == r_loader_name,
            (Self::AssetLoaderError(l0), Self::AssetLoaderError(r0)) => {
                l0.to_string() == r0.to_string()
            }
            (Self::AddAsyncError(l0), Self::AddAsyncError(r0)) => l0.to_string() == r0.to_string(),
            (
                Self::MissingLabel {
                    base_path: l_base_path,
                    label: l_label,
                    all_labels: l_all_labels,
                },
                Self::MissingLabel {
                    base_path: r_base_path,
                    label: r_label,
                    all_labels: r_all_labels,
                },
            ) => l_base_path == r_base_path && l_label == r_label && l_all_labels == r_all_labels,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl PartialEquality for RecursiveDependencyLoadState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Failed(l0), Self::Failed(r0)) => l0.eq(r0),
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
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
        if !current.eq(&tx.borrow()) && tx.send(current).is_err() {
            // Channel is closed, unsubscribe this asset
            to_unsubscribe.push(*id);
        }
    }
    for id in to_unsubscribe {
        subscriptions.handles.remove(&id);
    }
}

/// Manages interest in assets. Maintains a [`tokio::sync::watch::Sender`] for each asset
/// handle a client has expressed interest in. [`AssetSubscriptions::subscribe_to`] is used
/// to express interest in the load state for a given asset.
#[derive(Default, Resource)]
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
