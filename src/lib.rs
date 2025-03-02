use std::{
    future::Future,
    marker::Send,
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(feature = "asset")]
use async_asset::{notify_asset_events, AssetSubscriptions};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::{
    system::{Commands, Resource},
    world::World,
};
use bevy_tasks::AsyncComputeTaskPool;
use futures::FutureExt;
#[cfg(feature = "time")]
use time::time_plugin;
use tokio::sync::{mpsc, oneshot};

#[cfg(feature = "asset")]
pub mod async_asset;
pub mod common_uses;
pub mod event_stream;
#[cfg(feature = "time")]
pub mod time;

pub mod prelude {
    #[cfg(feature = "time")]
    pub use crate::time::TimingTaskExt;
    pub use crate::{
        common_uses::CommonUsesTaskExt, event_stream::EventStreamTaskExt, AsyncTasksPlugin,
        SpawnCommandExt, SpawnTaskExt, TaskContext,
    };
}

/// Adds [`AsyncWork`] resource to world to handle async jobs spawned from
/// [`TaskContext::with_world`], and schedules [`run_async_jobs`] in [`Update`] to dispatch
/// them.
pub struct AsyncTasksPlugin;

impl Plugin for AsyncTasksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AsyncWork>();
        app.add_systems(Update, run_async_jobs);
        #[cfg(feature = "asset")]
        {
            app.init_resource::<AssetSubscriptions>();
            app.add_systems(Update, notify_asset_events);
        }
        #[cfg(feature = "time")]
        app.add_plugins(time_plugin);
    }
}

/// This resource owns a queue for work that needs exclusive [`World`] access. Calling
/// [`create_task_context`] will give you a [`TaskContext`] that can be used to schedule
/// work onto the queue.
///
/// [`create_task_context`]: AsyncWork::create_task_context
#[derive(Resource)]
pub struct AsyncWork {
    work_tx: mpsc::UnboundedSender<Job>,
    work_rx: mpsc::UnboundedReceiver<Job>,
}

impl AsyncWork {
    /// Create a [`TaskContext`] which can schedule work onto this struct's
    /// queue. This work will be run next time [`run_async_jobs`] runs, which by
    /// default happens once per frame in [`Update`].
    pub fn create_task_context(&self) -> TaskContext {
        TaskContext {
            work_queue: self.work_tx.clone(),
        }
    }
}

impl Default for AsyncWork {
    fn default() -> Self {
        let (work_tx, work_rx) = mpsc::unbounded_channel();
        Self { work_tx, work_rx }
    }
}

/// This system dispatches jobs that need exclusive [`World`] access (any tasks created with
/// [`TaskContext::with_world`]). This system can be moved around to control how often and
/// when these tasks are dispatched.
pub fn run_async_jobs(world: &mut World) {
    let mut jobs = Vec::new();
    let mut work = world.resource_mut::<AsyncWork>();
    while let Ok(next) = work.work_rx.try_recv() {
        jobs.push(next);
    }
    for job in jobs {
        job(world);
    }
}

pub trait SpawnTaskExt {
    /// Spawn a task onto Bevy's async executor. The [`AsyncComputeTaskPool`] must have been
    /// initialized before this method is called (this is done automatically by [`TaskPoolPlugin`]).
    ///
    /// ```
    /// world.spawn_task(|cx| {
    ///     // Will spawn an entity once we have exclusive world access and
    ///     // return the id
    ///     let _spawned = cx.with_world(|world| world.spawn(()).id()).await;
    /// });
    /// ```
    ///
    /// [`TaskPoolPlugin`]: bevy::core::TaskPoolPlugin
    fn spawn_task<T, F>(&self, task: T)
    where
        T: FnOnce(TaskContext) -> F + Send + 'static,
        F: Future<Output = ()> + Send + 'static;
}

impl SpawnTaskExt for World {
    fn spawn_task<T, F>(&self, task: T)
    where
        T: FnOnce(TaskContext) -> F + Send + 'static,
        F: Future<Output = ()> + Send + 'static,
    {
        let context = self.resource::<AsyncWork>().create_task_context();
        AsyncComputeTaskPool::get().spawn(task(context)).detach();
    }
}

pub trait SpawnCommandExt {
    /// Spawn a task onto Bevy's async executor. The [`AsyncComputeTaskPool`] must be have been
    /// initialized before this command is applied (this is done automatically by
    /// [`TaskPoolPlugin`]).
    ///
    /// ```
    /// commands.spawn_task(|cx| {
    ///     // Will spawn an entity once we have exclusive world access and
    ///     // return the id
    ///     let _spawned = cx.with_world(|world| world.spawn(()).id()).await;
    /// });
    /// ```
    ///
    /// [`TaskPoolPlugin`]: bevy::core::TaskPoolPlugin
    fn spawn_task<T, F>(&mut self, task: T)
    where
        T: FnOnce(TaskContext) -> F + Send + 'static,
        F: Future<Output = ()> + Send + 'static;
}

impl SpawnCommandExt for Commands<'_, '_> {
    fn spawn_task<T, F>(&mut self, task: T)
    where
        T: FnOnce(TaskContext) -> F + Send + 'static,
        F: Future<Output = ()> + Send + 'static,
    {
        self.queue(move |world: &mut World| {
            world.spawn_task(task);
        });
    }
}

/// This is an adapter between async tasks and [`AsyncWork`]. This struct gets
/// passed as a paramter into all new async tasks and can be used to send work
/// to get run with exclusive world access. You can create one with
/// [`AsyncWork::create_task_context`], or this will be done for you when you
/// spawn a task with [`commands.spawn_task()`].
///
/// [`commands.spawn_task()`]: SpawnCommandExt::spawn_task
#[derive(Clone)]
pub struct TaskContext {
    work_queue: mpsc::UnboundedSender<Job>,
}

impl TaskContext {
    /// Execute a task with mutable world access. The task `f` is scheduled to
    /// be run the next time [`run_async_jobs`] is run, which by default happens
    /// once per frame in the [`Update`] schedule. For this reason, small tasks
    /// should be batched so they aren't scheduled with a frame delay between
    /// them.
    #[must_use = "Ignoring `with_world` return value. Either `.await` this value or `.detach()` it to run it in parallel"]
    pub fn with_world<R, F>(&self, f: F) -> WithWorld<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut World) -> R + Send + 'static,
    {
        WithWorld::new(f, &self.work_queue)
    }
}

pub struct WithWorld<R>(oneshot::Receiver<R>);

impl<R: Send + 'static> WithWorld<R> {
    fn new<F>(f: F, work_queue: &mpsc::UnboundedSender<Job>) -> Self
    where
        F: FnOnce(&mut World) -> R + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        work_queue
            .send(Box::new(move |world| {
                // If this `send` fails, most likely the user forgot to `await`
                // this future, and they should have a warning anyway, so we're
                // going to completely ignore this
                tx.send(f(world)).ok();
            }))
            .expect(
                "Failed to send task to `run_async_jobs`. Did you remove `AsyncWork` resource?",
            );
        Self(rx)
    }

    /// Discard the return value of this task and allow it to finish
    /// concurrently within the executor. This is useful for when your task does
    /// not return a value, e.g. when it simply mutates the world. This allows
    /// you to queue many tasks using `with_world` so they can potentially be
    /// dispatched within the same frame.
    ///
    /// ```
    /// cx.with_world(|world| {
    ///     world.entity_mut(e).insert(MyComponent);
    /// })
    /// .detach();
    /// ```
    pub fn detach(self) {}
}

impl<R> Future for WithWorld<R> {
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Sender end should never be dropped, safe to unwrap here
        self.0.poll_unpin(cx).map(Result::unwrap)
    }
}

type Job = Box<dyn FnOnce(&mut World) + Send>;
