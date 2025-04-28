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
        let loading_screen = cx
            .with_world(|world| {
                world
                    .spawn((
                        Node {
                            align_self: AlignSelf::Center,
                            justify_self: JustifySelf::Center,
                            ..default()
                        },
                        children![(
                            Text::new("Loading..."),
                            TextFont {
                                font_size: 36.0,
                                ..default()
                            },
                        )],
                    ))
                    .id()
            })
            .await;
        let scene = cx.load_asset("FlightHelmet.gltf#Scene0").await.unwrap();
        // Now that the scene is loaded, despawn the loading screen and spawn the scene in.
        // Unfortunately the scene won't appear right away because graphics pipelines need to be
        // compiled. I'd like to add an async API for awaiting pipeline compilation in the future.
        // This is partially blocked on Bevy, because while it's possible to query for pipeline
        // states, the API is particularly awkward and not very ergonomic at the moment.
        cx.despawn(loading_screen).detach();
        cx.spawn(SceneRoot(scene)).detach();
    });
}
