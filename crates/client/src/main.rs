//! Plays animations from a skinned glTF.

use bevy::math::vec4;
use bevy::{
    pbr::CascadeShadowConfigBuilder,
    prelude::*,
    utils::{HashMap, Instant},
};
use bevy_mod_picking::prelude::*;
use std::f32::consts::PI;
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(low_latency_window_plugin()),
            DefaultPickingPlugins.build(), //.disable::<DebugPickingPlugin>(),
        ))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1.0,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (setup_scene_once_loaded, keyboard_animation_control),
        )
        .add_systems(Update, make_pickable)
        .run();
}

#[derive(Resource)]
struct Animations(Vec<Handle<AnimationClip>>);

#[derive(Resource)]
struct Models {
    pub models: HashMap<&'static str, Handle<Scene>>,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Insert a resource with the current scene information
    commands.insert_resource(Animations(vec![asset_server.load("mouse.glb#Animation0")]));

    // Camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 1.0, 4.0).looking_at(Vec3::new(0.0, 0.2, 0.0), Vec3::Y),
        ..default()
    });

    // Light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        cascade_shadow_config: CascadeShadowConfigBuilder {
            first_cascade_far_bound: 200.0,
            maximum_distance: 400.0,
            ..default()
        }
        .into(),
        ..default()
    });
    let models = Models {
        models: HashMap::from([
            ("mouse".into(), asset_server.load("mouse.glb#Scene0")),
            ("mouse".into(), asset_server.load("mouse.glb#Scene0")),
        ]),
    };

    // Mouse
    commands.spawn((
        SceneBundle {
            scene: models.models["mouse"].clone_weak(),
            transform: Transform::from_scale(Vec3::splat(1f32))
                .with_translation(Vec3::new(-0.5f32, 0f32, 0f32)),
            //visibility: Visibility::Hidden,
            ..default()
        },
        Champion,
        Points(0),
        On::<Pointer<Click>>::target_component_mut::<Points>(|event, points| {
            info!("Clicked on entity {:?}", event.target);
            points.0 += 1;
        }),
    ));
    commands.spawn((
        SceneBundle {
            scene: models.models["mouse"].clone_weak(),
            transform: Transform::from_scale(Vec3::splat(1f32))
                .with_translation(Vec3::new(0.5f32, 0f32, 0f32)),
            //visibility: Visibility::Hidden,
            ..default()
        },
        PickableBundle::default(),
        Champion,
        Points(0),
        On::<Pointer<Click>>::target_component_mut::<Points>(|event, points| {
            info!("Clicked on entity {:?}", event.target);
            points.0 += 1;
        }),
    ));
}

const HIGHLIGHT_TINT: Highlight<StandardMaterial> = Highlight {
    hovered: Some(HighlightKind::new_dynamic(|matl| StandardMaterial {
        base_color: matl.base_color + vec4(-0.5, -0.3, 0.9, 0.8), // hovered is blue
        ..matl.to_owned()
    })),
    pressed: Some(HighlightKind::new_dynamic(|matl| StandardMaterial {
        base_color: matl.base_color + vec4(-0.4, -0.4, 0.8, 0.8), // pressed is a different blue
        ..matl.to_owned()
    })),
    selected: Some(HighlightKind::new_dynamic(|matl| StandardMaterial {
        base_color: matl.base_color + vec4(-0.4, 0.8, -0.4, 0.0), // selected is green
        ..matl.to_owned()
    })),
};
fn make_pickable(
    mut commands: Commands,
    meshes: Query<Entity, (With<Handle<Mesh>>, Without<Pickable>)>,
) {
    for entity in meshes.iter() {
        dbg!("yo");
        commands
            .entity(entity)
            .insert((PickableBundle::default(), HIGHLIGHT_TINT.clone()));
    }
}
// Once the scene is loaded, start the animation
fn setup_scene_once_loaded(
    animations: Res<Animations>,
    mut players: Query<(&mut AnimationPlayer, &mut Visibility), Added<AnimationPlayer>>,
) {
    for mut player in &mut players {
        //player.0.play(animations.0[0].clone_weak()).repeat();
        //*player.1 = Visibility::Inherited;
    }
}

fn keyboard_animation_control(
    keyboard_input: Res<Input<KeyCode>>,
    mut animation_players: Query<&mut AnimationPlayer>,
    animations: Res<Animations>,
    mut current_animation: Local<usize>,
) {
    for mut player in &mut animation_players {
        if keyboard_input.just_pressed(KeyCode::Return) {
            *current_animation = (*current_animation + 1) % animations.0.len();
            player
                .play_with_transition(
                    animations.0[*current_animation].clone_weak(),
                    Duration::from_millis(250),
                )
                .repeat();
        }
    }
}

fn update_scale_with_points(mut to_scale: Query<(&mut Transform, &Points), Changed<Points>>) {
    for (mut scale, points) in to_scale.iter_mut() {
        scale.scale = Vec3::splat(1f32 + points.0 as f32 * 0.1);
    }
}

#[derive(Component)]
pub struct Points(pub i32);

/// Marker, has Points.
#[derive(Component)]
pub struct SharedPool;

/// Marker, has Points.
#[derive(Component)]
pub struct Champion;

#[derive(Component)]
pub struct Cooldown {
    pub ready_at: Instant,
}
