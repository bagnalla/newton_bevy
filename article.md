# Bevy's Gift

## Intro

I recently became aware of a game programming framework for Rust
called [Bevy](https://bevyengine.org/), which uses a programming model
called Bevy ECS (Entity Component System) that can supposedly use
Rust's type system in a sophisticated way to automatically parallelize
execution of game logic while avoiding race conditions. The idea of
ECS has apparently been around for a while (at least as far back as
2007 [according to
Wikipedia](https://en.wikipedia.org/wiki/Entity_component_system#History)),
but it was new to me.

Curious to see if Bevy is really as cool as it sounds, and having
recently read [Newton's
Gift](https://www.goodreads.com/book/show/267406.Newton_s_Gift), I
decided to try using Bevy to implement an n-body simulation (i.e.,
simulating the gravitational dynamics of a small galaxy of massive
bodies) with elastic collisions (the bodies can bounce off one
another). This article steps through the implementation and shows how
with a little bit of ingenuity we can get a significant speedup by
exploiting Bevy's automatic parallelization feature. The code can be
found [here](https://github.com/bagnalla/newton_bevy).

DISCLAIMER: I'm not a contributor to Bevy nor am i associated with the
project in any way. I give no guarantee that the following code is
idiomatic or even sensible. Everything in this article could be
wrong. I just wanted to share my experience because I was really
amazed at how it turned out. There are several demos out there of
n-body simulations using Bevy, but none of them seem to focus on what
to me is the most interesting part: automatic parallelization.

## ECS

First, we need to know the basic concepts of ECS. The [Bevy
tutorial](https://bevyengine.org/learn/book/getting-started/ecs/) says
the following:

> ``` ECS is a software pattern that involves breaking your program up into Entities, Components, and Systems. Entities are unique "things" that are assigned groups of Components, which are then processed using Systems. ```

In other words:

* Entity - Every game object is an entity with a unique numeric
  identifier. An entity may be nothing more than an identifier, but it
  may also have some associated *components*.

* Component - A component is a data field that can be associated with
  an entity, e.g., position (a component of type Vec3 if working in 3D
  space), velocity (also of type Vec3), hit points (integer), mass
  (float), etc.

* System - A system is a chunk of code that runs once per update cycle
  (i.e., each "frame") with reference to a specific collection of
  entities. The collection of entities visible to a system is
  specified by a query, where the semantics of the query is determined
  by its type. These queries are where things get really interesting,
  because their type information can be used by Bevy to schedule
  systems to run in parallel when it can guarantee the absence of race
  conditions from looking at the types of their queries.

## N-body Simulation with Bevy ECS

Before worrying about parallelization, let's set up the basic n-body
simulation.

### Components

We start by defining all the components we need:

```rust
#[derive(Component)]
struct Velocity(Vec3);

#[derive(Component)]
struct Radius(f32);

#[derive(Component)]
struct Mass(f32);
```

Each of our body objects will have a `Velocity` component, a `Radius`
component, and a `Mass` component. Velocity is a vector with three
`f32` elements, and radius and mass are both `f32` scalar values.

### Systems

The simulation works by executing the following steps on every frame:

1) move each body based on its current velocity,

2) calculate gravitational forces between every pair of bodies and
apply the corresponding acceleration (change of velocity), and

3) check for collisions between every pair of bodies and handle them
when they occur by adjusting the positions and velocities of the
affected bodies.

Obviously, the running time of steps 2) and 3) scale quadratically in
the number of bodies, so we will be limited in the number of bodies
the simulation can support (no more than a couple thousand). There are
[ways to improve on
this]([](https://en.wikipedia.org/wiki/Barnes%E2%80%93Hut_simulation)),
but we'll be keeping things simple.

We implement a system for each step of the simulation in turn,
starting with step 1:

```rust
fn move_system(time: Res<Time>,
	       mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, &Velocity(v)) in query.iter_mut() {
	transform.translation += v * time.delta_seconds()
    }
}
```

The thing to notice is the type of `query`: `Query<(&mut Transform,
&Velocity)>`. It says to select all entities that have both a
`Transform` and a `Velocity` component, taking a mutable (writeable)
reference to the transform and an immutable (readonly) reference to
the velocity. The system's code simply iterates over all of the
entities given by the query and updates their positions based on their
current velocity (scaled by the time elapsed since the previous frame
to make on-screen velocity independent of framerate).

Next up is the system for applying gravitational forces:

```rust
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
```

The gravitational constant `G` is a parameter that controls the
strength of gravity. The query is similar to that of `move_system`
except we also include the `Mass` component since the effect of
gravity depends on the mass of the bodies involved, we take a mutable
reference to the velocity component since we are modifying the
velocities of bodies, and we take an immutable reference to the
transform component since we are not changing the positions of
bodies. The code iterates over all pairs of bodies and adjusts their
velocities based via an [inverse square law of
attraction](https://en.wikipedia.org/wiki/Inverse-square_law) based on
the distance between them and their relative masses.

Next is the collision system:
```rust
fn collision_system(mut query: Query<(&mut Transform, &mut Velocity, &Radius, &Mass)>) {
    let mut combinations = query.iter_combinations_mut();
    while let Some([(mut a_transform, mut a_velocity, Radius(a_radius), Mass(a_mass)),
		    (mut b_transform, mut b_velocity, Radius(b_radius), Mass(b_mass))]) =
    	combinations.fetch_next() {
	    let v = a_transform.translation - b_transform.translation;
    	    let d = (a_radius + b_radius) - v.length();
    	    if d > 0.0 {
			// Collision detected - compute new positions and velocities
	    }
	}
}
```

We omit the code for actually handling collisions when they occur
because it detracts from the main point: the query. The key thing to
notice about the query is that it takes a reference to the velocity
component of bodies. The gravity system, if you recall, takes a
*mutable* reference to the velocity component. This means,
unfortunately, that it would be unsafe to execute the gravity and
collision systems in parallel because there is a potential race
condition when the gravity system writes to the velocity component at
the same time that the collision system reads from it. In fact,
`collision_system`'s reference to velocity is also mutable, so the two
systems could even try to write to it at the same time. Bevy's system
scheduler is forced to schedule them in sequence because it can't
guarantee the absence of race conditions from looking at their types.

### Initialization

Lastly, we need to initialize the Bevy engine (including registering
our systems with the scheduler) and kick off the simulation:

```rust
fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
	.init_resource::<State>()
	.insert_resource(ClearColor(Color::BLACK))
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
        .run();
}
```

We've left out some of the setup functions, e.g., `setup_camera` and
`setup_bodies` because they aren't important for the
demonstration. Just know that in `setup_bodies` we create two large
"planet" entities with fixed initial positions and velocities and many
smaller bodies (2000 of them) with fixed initial positions but small
random initial velocities. When registering our systems, we tell the
scheduler to ensure that the collision and gravity systems run *after*
the move system, but otherwise it can order them however it likes.

The result:

TODO: video/gif

Pretty cool, huh? Yes, the planets are spontaneously forming
rings. That was not planned. Anyway, as cool as the simulation is, the
framerate is far from perfect, at around 21 or 22 frames per second on
my machine. We can improve on this by reengineering our systems a bit
so that Bevy's scheduler can get away with running the collision and
gravity systems in parallel.

## Exploiting Automatic Parallelism

How can we possibly convince the Bevy scheduler that it's okay to run
the collision and gravity systems at the same time, when they both
require references (both mutable references at that) to the velocity
component? Our solution is based on a simple observation: most bodies,
most of the time, are *not* colliding with one another (i.e, on most
frames any given body is not participating in any collision at all,
and even when it does it is with at most a couple other colliding
bodies). This means that the mutable reference to the velocity
component is rarely used in proportion to the total number of
collision checks that are performed. The number of checks is a
constant on the order of `n^2` every frame, whereas the number of
actual collisions is on average *much* smaller than that.

We can remove the reference to the velocity component from the
collision checking system altogether and have it build a queue of
collisions to be handled at a later stage, thereby allowing the bulk
of the work to be performed in parallel with the gravity system. We
start by defining a struct for collision records to be pushed to the
queue:

```rust
struct Collision(Entity, Entity);
```

Then we remove `&mut Velocity` from the collision system's query type
(and add `Entity` to include entity IDs and remove `Mass` because it's
no longer needed) and include a new `EventWriter<Collision>` argument:

```rust
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
```

Now when a collision is detected we simply send a `Collision` event
through the `EventWriter` recording the pair of colliding objects. We
then write a new system for handling detected collisions:


```rust
fn collision_handler_system(mut bodies: Query<(&mut Transform, &mut Velocity, &Radius, &Mass)>,
			    mut collisions: EventReader<Collision>) {
    for &Collision(a_id, b_id) in collisions.iter() {
	let (a_transform, a_velocity, &Radius(a_radius), &Mass(a_mass)) =
	    bodies.get(a_id).unwrap();
	let (b_transform, b_velocity, &Radius(b_radius), &Mass(b_mass)) =
	    bodies.get(b_id).unwrap();
	let v = a_transform.translation - b_transform.translation;
    	let d = a_radius + b_radius - v.length();
		// Handle collision
    }
}
```

We could slightly optimize further by including the values of `d` and
`v` in the `Collision` struct since they are already computed in
`collision_system`, but it makes little difference because, as we've
already noted, collisions are relatively rare events.

Lastly, we update the initialization code to register the Collision
event and new collision handler system (specified to run *after* the
collision checker system):

```rust
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
```

The result:

TODO: video/gif

The same as before, but now at around 44 frames per second (a 1.5x
speedup!). As a sanity check, we can change `collision_system`'s
reference to the transform component to be mutable, and we're right
back to where we started at 22 frames per second because Bevy can
longer run it in parallel with `gravity system`.

-- Alex Bagnall <abagnalla@gmail.com>