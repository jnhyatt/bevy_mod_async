use bevy::{input::keyboard::KeyboardInput, prelude::*};
use bevy_mod_async::prelude::*;
use futures::StreamExt;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AsyncTasksPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera3d::default());
    // `spawn_task` is a helper to spawn an async task onto the Bevy executor with a
    // `TaskContext` instance to get mutable world access
    commands.spawn_task(|cx| async move {
        // `TaskContext` provides APIs built on top of `with_world` to simplify common tasks

        // For example, `TaskContext::spawn` is shorthand for
        // `cx.with_world(|world| world.spawn(...).id())`
        let container = cx
            .spawn(Node {
                justify_self: JustifySelf::Center,
                align_self: AlignSelf::Center,
                ..default()
            })
            .await;

        // We can't use `cx.spawn` here because we need to use `set_parent`
        // Improved constructors in bevy 0.16 sould make this more ergonomic
        let text_entity = cx
            .with_world(move |world| {
                world
                    .spawn((
                        Text::new("Waiting for keyboard event"),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                    ))
                    .set_parent(container)
                    .id()
            })
            .await;

        // `event_stream` returns a `Stream` over any (clonable) event type
        let mut events = cx.event_stream::<KeyboardInput>();
        while let Some(ev) = events.next().await {
            // `bevy_mod_async`'s primary API is `with_world`. Every other method provided
            // on `TaskContext` is built on top of it. Essentially, it moves the provided
            // closure onto the main thread and executes it once exclusive world access is
            // available, then provides you a `Future` that completes when the operation
            // does and returns its result
            cx.with_world(move |world| {
                // You can do anything in here that you could with a `&mut World` -- this
                // closure runs essentially as an exclusive system, so you can spawn
                // entities, access their components, resources, events, etc. The result of
                // the closure will be passed back to your async task
                let mut e = world.entity_mut(text_entity);
                let mut text = e.get_mut::<Text>().unwrap();
                text.0 = format!("Got keyboard event: {:?}", ev.key_code);
            })
            .detach();
        }
    });
}
