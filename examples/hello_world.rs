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

        // `bevy_mod_async`'s primary API is `with_world`. Every other method provided
        // on `TaskContext` is built on top of it. Essentially, it moves the provided
        // closure onto the main thread and executes it once exclusive world access is
        // available, then provides you a `Future` that completes when the operation
        // does and returns its result
        let text_entity = cx
            .spawn((
                Text::new("Waiting for keyboard event"),
                TextFont {
                    font_size: 36.0,
                    ..default()
                },
                ChildOf(container),
            ))
            .await;

        // `event_stream` returns a `Stream` over any (clonable) event type
        let mut messages = cx.message_stream::<KeyboardInput>();
        while let Some(m) = messages.next().await {
            // `cx.with_entity` is a helper that wraps `with_world` and gives you an
            // `EntityWorldMut`to do whatever you want with:
            cx.with_entity(text_entity, move |mut text_entity| {
                let mut text = text_entity.get_mut::<Text>().unwrap();
                text.0 = format!("Got keyboard message: {:?}", m.key_code);
            })
            .detach();
        }
    });
}
