use std::{future::Future, time::Duration};

use bevy_app::{App, Update};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    system::{Commands, Query, Res},
    world::World,
};
use bevy_time::Time;
use futures::TryFutureExt;
use tokio::sync::oneshot;

use crate::TaskContext;

pub fn time_plugin(app: &mut App) {
    app.add_systems(Update, (advance_timeout_after, advance_timeout_at));
}

pub trait TimingTaskExt {
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()>;
    fn sleep_until(&self, duration: Duration) -> impl Future<Output = ()>;
}

impl TimingTaskExt for TaskContext {
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> {
        let (tx, rx) = oneshot::channel();
        self.with_world(move |world| {
            world.spawn(TimeoutAfter(duration, tx));
        })
        .detach();
        rx.unwrap_or_else(|_| ())
    }

    fn sleep_until(&self, elapsed_since_startup: Duration) -> impl Future<Output = ()> {
        let (tx, rx) = oneshot::channel();
        self.with_world(move |world| {
            world.spawn(TimeoutAt(elapsed_since_startup, tx));
        })
        .detach();
        rx.unwrap_or_else(|_| ())
    }
}

#[derive(Component)]
pub struct TimeoutAfter(Duration, oneshot::Sender<()>);

#[derive(Component)]
pub struct TimeoutAt(Duration, oneshot::Sender<()>);

pub fn advance_timeout_after(
    mut timeouts: Query<(Entity, &mut TimeoutAfter)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut timeout) in &mut timeouts {
        if let Some(new_timeout) = timeout.0.checked_sub(time.delta()) {
            timeout.0 = new_timeout;
        } else {
            commands.queue(move |world: &mut World| {
                let Ok(mut e) = world.get_entity_mut(e) else {
                    return;
                };
                if let Some(timeout) = e.take::<TimeoutAfter>() {
                    timeout.1.send(()).ok();
                }
                e.despawn();
            });
        }
    }
}

pub fn advance_timeout_at(
    timeouts: Query<(Entity, &TimeoutAt)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, timeout) in &timeouts {
        if time.elapsed() >= timeout.0 {
            commands.queue(move |world: &mut World| {
                let Ok(mut e) = world.get_entity_mut(e) else {
                    return;
                };
                if let Some(timeout) = e.take::<TimeoutAt>() {
                    timeout.1.send(()).ok();
                }
                e.despawn();
            });
        }
    }
}
