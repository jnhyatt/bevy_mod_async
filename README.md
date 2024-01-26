# `bevy_mod_async`
`bevy_mod_async` is my attempt at a more ergonomic API for async tasks in Bevy. It's built on `bevy_tasks`'s executor, so there's no need to pull in another async runtime (although `bevy_mod_async` does use some `tokio` types for its async interface -- which could be replaced in the future).

## Usage

Adding `AsyncTasksPlugin` ensures all async tasks will be properly updated, and adds the resources `TaskContext` needs to spawn in tasks:
```rs
use bevy_mod_async::prelude::*;
...
app.add_plugins(AsyncTasksPlugin);
```
After that, `bevy_mod_async` has two primary APIs: `commands.spawn_task()` (taking an async closure with a single argument of type `TaskContext`):
```rs
commands.spawn_task(|cx| async move {
    ...
});
```
and `TaskContext::with_world`, which is used to exclusively, asynchronously access the world:
```rs
cx.with_world(|world| {
    // do anything with `world` here as in an exclusive system
    let e = world.spawn(()).id();
    world.despawn(e);
    world.resource_mut::<Counter>().0 += 1;
}).await;
```
Several convenience methods are provided as extensions on top of this API:
```rs
let e = cx.spawn(()).await;
cx.despawn(e).await;
let a = cx.load_asset::<Mesh>("model.glb#Mesh0").await.unwrap();
```
Many of these APIs return `WithWorld`, a `Future` that, when awaited, returns the result of executing the command on the `World`. This means that after `.await`ing the future, any modifications to the world will have taken effect. Due to the fact that `WithWorld` futures are (by default) only advanced once per frame, it also means that each `.await` will typically delay execution by one frame. If this is undesirable, the task can be detached as well:
```rs
cx.spawn(()).detach();
```
This will still push the task onto Bevy's executor, but it will not suspend execution (which also obviously means the world won't have been modified either).

## Motivation

What's wrong with vanilla `bevy_tasks`? Well, Bevy's primary API for kicking off async tasks uses `AsyncTaskPool`:
```rs
let task = AsyncComputeTaskPool::get().spawn(my_task);
```
Once you've spawned a new task, the executor takes care of polling the resulting future to completion. But how do you access the result? You've got to do your own polling, usually using a Bevy system:
```rs
struct MyTask(Task<..>);
fn handle_task_completion(mut tasks: Query<&mut MyTask>, mut commands: Commands) {
    for mut task in &mut tasks {
        if task.is_finished() {
            let result = block_on(poll_once(&mut task.0));
            // handle result
        }
    }
}
```
If you're spawning lots of similar async tasks, that's not too bad. For a one-shot async task like reacting to an asset loading, that's a lot of boilerplate. Instead, you could handle the async task finishing inside an `async` block:
```rs
AsyncComputeTaskPool::get().spawn(async move {
    let result = my_task.await;
    println!("{result}");
}).detach();
```
Of course, this approach only works if you can comfortably handle your result without access to the ECS, since the task has no way to access the `World` by itself. Another approach if you wanted to avoid making a task-specific system, you could instead use channels to send tasks to a task handler system and return their results. This is the approach `bevy_mod_tasks` uses. There's an exclusive system that runs queued tasks, a resource that accumulates them, and channels that pass tasks and results back and forth between this exclusive system and your tasks.

This comes with obvious limitations: all your `World` accesses have to wait until the next time this exclusive system runs to get their results back, and none of the accesses are parallelizable. On the other hand, if you're doing something as simple as loading an asset, waiting until it's finished loading, then spawning it into the scene, `bevy_mod_async` gives you a simple API to write this logic in an easy-to-read, linear way. The performance impact in this case is negligible: the task is only running once, and it's running at load time, rather than in a hot game loop.

## Examples

### [`hello_world`](examples/hello_world.rs)
Demonstrates some of `TaskContext`'s async APIs, and tries to simply explain how it works internally. Shows how to subscribe to an asynchronous event stream and basic UI reactivity.

### [`async_asset`](examples/async_asset.rs)
Demonstrates loading assets asynchronously. Spawns a loading screen, then despawns it when the scene is ready to be loaded.

## Bevy Version

| bevy | bevy_mod_async |
| ---- | -------------- |
| 0.12 | 0.1-0.2        |

## Contribution

PRs welcome. I'll admit I'm not the world's leading expert in asynchronous Rust programming, nor do I have an incredible grasp of Bevy's ECS internals. If anyone has performance, ergonomics, or other improvements in mind, I'm open to contributions.
