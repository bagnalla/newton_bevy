use rand::prelude::*;

use bevy::{
    prelude::*,
    input::{keyboard::KeyCode, Input},
    input::mouse::{MouseMotion},
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin}
};

struct State {
    cam_speed: f32
}

impl Default for State {
    fn default() -> State {
	State {
	    cam_speed: 1.0
	}
    }
}

#[derive(Component)]
struct Velocity(Vec3);

#[derive(Component)]
struct Radius(f32);

#[derive(Component)]
struct Mass(f32);

struct Collision(Entity, Entity);

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
	.init_resource::<State>()
	.insert_resource(ClearColor(Color::BLACK))
	.add_event::<Collision>()
        .add_plugins(DefaultPlugins)
	.add_plugin(LogDiagnosticsPlugin::default())
	.add_plugin(FrameTimeDiagnosticsPlugin::default())
	.add_startup_system(setup_light)
        .add_startup_system(setup_camera)
	.add_startup_system(setup_bodies)
	.add_system(update_camera)
	.add_system(move_system.label("move"))
	.add_system(collision_system.label("collision").after("move"))
	.add_system(gravity_system.after("move"))
	.add_system(collision_handler_system.after("collision"))
        .run();
}

fn setup_light(mut commands: Commands) {
    // ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0
    });
}

fn setup_camera(mut commands: Commands) {
    commands.spawn_bundle(PerspectiveCameraBundle {
        // transform: Transform::from_xyz(0.0, 0.0, 5.0)
	transform: Transform::from_xyz(0.0, 0.0, 25.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}

fn setup_bodies(mut commands: Commands,
		mut meshes: ResMut<Assets<Mesh>>,
		mut materials: ResMut<Assets<StandardMaterial>>) {

    let sphere_handle = meshes.add(Mesh::from(shape::Icosphere {
	radius: 1.0,
	subdivisions: 4
    }));
    let sphere_material_handle = materials.add(StandardMaterial {
	base_color: Color::rgb(0.8, 0.7, 0.6),
	..Default::default()
    });

    commands.spawn_bundle(PbrBundle {
	..Default::default()
    })
	.with_children(|u| {
	    let mut rng = rand::thread_rng();

	    // Create two large "planets" with fixed initial positions
	    // and velocities.
	    let radius = 1.0;
	    let volume = 4.0 * std::f32::consts::PI * f32::powf(radius, 3.0) / 3.0;
	    u.spawn_bundle(PbrBundle {
	    	mesh: sphere_handle.clone(),
	    	material: sphere_material_handle.clone(),
	    	transform: Transform::from_scale(Vec3::splat(radius))
	    	    .with_translation(Vec3::new(0.0, 5.0, 0.0)),
	    	..Default::default()
	    })
	    	.insert(Velocity(-Vec3::X * 0.75))
	    	.insert(Mass(volume))
	    	.insert(Radius(radius));

	    u.spawn_bundle(PbrBundle {
	    	mesh: sphere_handle.clone(),
	    	material: sphere_material_handle.clone(),
	    	transform: Transform::from_scale(Vec3::splat(radius))
	    	    .with_translation(Vec3::new(0.0, -5.0, 0.0)),
	    	..Default::default()
	    })
	    	.insert(Velocity(Vec3::X * 0.75))
	    	.insert(Mass(volume))
	    	.insert(Radius(radius));

	    // Create many smaller bodies with fixed initial positions
	    // but random small initial velocities.
	    for _ in 0..2000 {
	    	let speed = 1.0;
	    	let radius = 0.01 + 0.1 * rng.gen::<f32>();
		let volume = 4.0 * std::f32::consts::PI * f32::powf(radius, 3.0) / 3.0;
	    	u.spawn_bundle(PbrBundle {
	    	    mesh: sphere_handle.clone(),
	    	    material: sphere_material_handle.clone(),
	    	    transform: Transform::from_scale(Vec3::splat(radius))
	    		.with_translation(Vec3::new(rng.gen::<f32>() * 5.0 - 2.5,
	    					    rng.gen::<f32>() * 5.0 - 2.5,
	    					    rng.gen::<f32>() * 5.0 - 2.5)),
	    	    ..Default::default()
	    	})
	    	    .insert(Velocity(Vec3::new(rng.gen::<f32>() * speed,
	    	    			       rng.gen::<f32>() * speed,
	    	    			       rng.gen::<f32>() * speed)))
	    	    .insert(Mass(volume))
	    	    .insert(Radius(radius));
	    }
	});
}

fn update_camera(time: Res<Time>,
		 mut state: ResMut<State>,
		 keyboard_input: Res<Input<KeyCode>>,
		 mut transforms: Query<(&mut Transform, With<Camera>)>,
		 mut mouse_motion_events: EventReader<MouseMotion>) {
    let (mut transform, _) = transforms.iter_mut().next().unwrap();    
    let mut translation = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::W) {
    	translation += transform.forward()
    }
    if keyboard_input.pressed(KeyCode::A) || keyboard_input.pressed(KeyCode::Q) {
    	translation += transform.left()
    }
    if keyboard_input.pressed(KeyCode::S) {
    	translation += transform.back()
    }
    if keyboard_input.pressed(KeyCode::D) || keyboard_input.pressed(KeyCode::E) {
    	translation += transform.right()
    }
    if keyboard_input.pressed(KeyCode::Space) {
    	translation += transform.up()
    }
    if keyboard_input.pressed(KeyCode::Z) {
    	translation += transform.down()
    }
    transform.translation += translation * state.cam_speed * time.delta_seconds();
    if keyboard_input.just_pressed(KeyCode::PageUp) {
    	state.cam_speed += 1.0
    }
    if keyboard_input.just_pressed(KeyCode::PageDown) {
    	state.cam_speed -= 1.0
    }
    for event in mouse_motion_events.iter() {
    	let left = transform.left();
    	let down = transform.down();
    	let mut forward = transform.forward();
    	forward = Quat::from_axis_angle(left, event.delta.y * time.delta_seconds())
    	    .mul_vec3(forward);
    	forward = Quat::from_axis_angle(down, event.delta.x * time.delta_seconds())
    	    .mul_vec3(forward);	
    	let pos = transform.translation;
    	transform.look_at(pos + forward, Vec3::Y);
    }
}

/// System for translating bodies by their current velocities.
fn move_system(time: Res<Time>,
	       mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, &Velocity(v)) in query.iter_mut() {
	transform.translation += v * time.delta_seconds()
    }
}

fn collision_system(query: Query<(Entity, &Transform, &Radius)>,
		    mut collisions: EventWriter<Collision>) {
    for [(a_id, a_transform, &Radius(a_radius)),
	 (b_id, b_transform, &Radius(b_radius))] in query.iter_combinations() {
	let v = a_transform.translation - b_transform.translation;
    	let d = a_radius + b_radius - v.length();
    	if d > 0.0 {
    	    collisions.send(Collision(a_id, b_id))
    	}
    }
}

fn collision_handler_system(mut bodies: Query<(&mut Transform, &mut Velocity, &Radius, &Mass)>,
			    mut collisions: EventReader<Collision>) {
    for &Collision(a_id, b_id) in collisions.iter() {
	let (a_transform, a_velocity, &Radius(a_radius), &Mass(a_mass)) =
	    bodies.get(a_id).unwrap();
	let (b_transform, b_velocity, &Radius(b_radius), &Mass(b_mass)) =
	    bodies.get(b_id).unwrap();
	let v = a_transform.translation - b_transform.translation;
    	let d = a_radius + b_radius - v.length();
	let dir = v.normalize();
	let ds = v.length_squared();
	let mass_ratio = b_mass / (a_mass + b_mass);
	let new_a_translation = a_transform.translation + dir * mass_ratio * d;
	let new_b_translation = b_transform.translation - dir * (1.0 - mass_ratio) * d;
	let new_a_velocity =
	    Velocity(a_velocity.0 - (2.0 * b_mass / (a_mass + b_mass)) *
		     ((a_velocity.0 - b_velocity.0).dot(v) / ds) * v);
	let new_b_velocity =
	    Velocity(b_velocity.0 - (2.0 * a_mass / (a_mass + b_mass)) *
		     ((b_velocity.0 - a_velocity.0).dot(-v) / ds) * -v);
	let (mut transform, mut velocity, _, _) = bodies.get_mut(a_id).unwrap();
	transform.translation = new_a_translation;
	*velocity = new_a_velocity;
	let (mut transform, mut velocity, _, _) = bodies.get_mut(b_id).unwrap();
	transform.translation = new_b_translation;
	*velocity = new_b_velocity
    }
}

const G: f32 = 1.0;
fn gravity_system(time: Res<Time>,
		  mut query: Query<(&Transform, &mut Velocity, &Mass)>) {
    let mut combinations = query.iter_combinations_mut();
    while let Some([(a_transform, mut a_velocity, &Mass(a_mass)),
    		    (b_transform, mut b_velocity, &Mass(b_mass))]) =
    	combinations.fetch_next() {
    	    let v = a_transform.translation - b_transform.translation;
    	    let dir = v.normalize() * G / f32::powf(v.length(), 2.0) * time.delta_seconds();
	    a_velocity.0 -= b_mass * dir;
    	    b_velocity.0 += a_mass * dir
    	}
}
