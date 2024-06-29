use std::time::Duration;

use bevy::prelude::*;
use bevy_mod_async::{time::TimingTaskExt, AsyncTasksPlugin, SpawnCommandExt};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(), // This is required if the `asset` feature is enabled
            AsyncTasksPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn_task(|cx| async move {
        let mut counter = 0;
        loop {
            cx.sleep(Duration::from_secs(1)).await;
            println!("Counter: {counter}");
            counter += 1;
        }
    });
}
