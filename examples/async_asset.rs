use bevy::prelude::*;
use bevy_mod_async::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AsyncTasksPlugin))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1_000.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn_task(|cx| async move {
        // Spawn in a nice loading screen
        let (container, text) = cx
            .with_world(|world| {
                let container = world
                    .spawn(Node {
                        align_self: AlignSelf::Center,
                        justify_self: JustifySelf::Center,
                        ..default()
                    })
                    .id();
                let text = world
                    .spawn((
                        Text::new("Loading..."),
                        TextFont {
                            font_size: 36.0,
                            ..default()
                        },
                    ))
                    .set_parent(container)
                    .id();
                (container, text)
            })
            .await;
        let scene = cx.load_asset("FlightHelmet.gltf#Scene0").await.unwrap();
        // Now that the scene is loaded, despawn the loading screen and spawn the scene in
        // nb we need to despawn each level of the hierarchy, as despawn_recursive isn't available on [World]
        cx.despawn(text).detach();
        cx.despawn(container).detach();
        cx.spawn(SceneRoot(scene)).detach();
    });
}
