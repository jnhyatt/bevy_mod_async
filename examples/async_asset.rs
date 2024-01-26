use std::future;

use bevy::prelude::*;
use bevy_asset::RecursiveDependencyLoadState;
use bevy_mod_async::prelude::*;
use futures::StreamExt;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AsyncTasksPlugin))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1.0,
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(2.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    commands.spawn_task(|cx| async move {
        // Spawn in a nice loading screen
        let loading_screen = cx
            .spawn(
                TextBundle::from_section(
                    "Loading...",
                    TextStyle {
                        font_size: 36.0,
                        ..default()
                    },
                )
                .with_style(Style {
                    align_self: AlignSelf::Center,
                    justify_self: JustifySelf::Center,
                    ..default()
                }),
            )
            .await;
        // Load a scene
        let scene = cx.load_asset::<Scene>("FlightHelmet.gltf#Scene0").await;
        // Wait until the next load state is a `Loaded`
        cx.get_load_state(scene.clone())
            .filter(|&x| future::ready(x == RecursiveDependencyLoadState::Loaded))
            .next()
            .await;
        // Now that the scene is loaded, despawn the loading screen and spawn the scene in
        cx.despawn(loading_screen).detach();
        cx.spawn(SceneBundle { scene, ..default() }).detach();
    });
}
