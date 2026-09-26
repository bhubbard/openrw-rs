# openrw-rs

[![GitHub Pages](https://img.shields.io/badge/docs-GitHub%20Pages-orange?style=flat-square&logo=github)](https://bhubbard.github.io/openrw-rs/)
[![Tests](https://img.shields.io/badge/tests-20%20passed-success?style=flat-square&logo=rust)](https://github.com/bhubbard/openrw-rs)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)](LICENSE-MIT)

Pure Rust port of core GTA III gameplay mechanics inspired by [OpenRW](https://github.com/rwengine/openrw), architected for modularity and modern ECS game engines such as [Bevy](https://bevyengine.org/).

## Features

### 1. Camera Follow System (`openrw_rs::camera`)
- **Third-Person Follow**: Behind-the-player camera with critically damped spring smoothing, pitch angle tuning, lateral shoulder offset, and dynamic speed-dependent pull-back.
- **Bumper / Hood Camera**: First-person and vehicle-mounted viewpoint with look offsets and rear mirror inversion.
- **Cinematic Chase Camera**: Dynamic multi-angle dramatic views (low ground tracking, wheel profile shot, high cinematic overview) that cycle during high-speed chases.
- **Mouse Orbit Camera**: Pitch / yaw spherical coordinate orbit with user input controls and zoom limits.
- **Collision Zoom Clipping**: Environment raycasting abstraction (`RaycastWorld`) preventing camera penetration through walls and terrain, clamping to a safe offset with configurable collision margins.

### 2. Weapon Targeting Math (`openrw_rs::targeting`)
- **Hitscan & Raycasting**: Precise direction raycasts with range clamping.
- **GTA Auto-Aim Prioritization**: Lock-on scoring evaluating candidate target distance, crosshair angle cone alignment, direct line-of-sight validation, and active hostile threat weighting.
- **Target Cycling**: Left/right target selection cycling across field-of-view candidates.
- **Cone of Fire & Bullet Spread**: Stance-based accuracy multipliers (Crouching, Standing, Walking, Running, InVehicle) with continuous-fire bloom heat buildup and linear recovery cooldown.
- **Ballistic Trajectories & Lead Targeting**: Quadratic trajectory solver calculating required lead angles and flight times to intercept moving targets with physical projectiles.

### 3. Player State Machine (`openrw_rs::player_state`)
- **ECS-Compatible States**: `Idle`, `Walk`, `Run`, `Sprint`, `EnterVehicle`, `InVehicle`, `ExitVehicle`, `Aim`, `Shoot`, `Jump`, `Fall`, `Land`, `Ragdoll`.
- **Validation & Recovery Timers**: Validated transitions preventing illegal state jumps (e.g. jumping while airborne, sprinting during ragdoll).
- **Physical Dynamics**: Fall height tracking, soft vs hard landing recovery locks, vehicle door entry timers, and high-speed bailouts triggering ragdoll tumbling.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
openrw-rs = { git = "https://github.com/bhubbard/openrw-rs" }
glam = "0.29"
```

## Quick Example

```rust
use glam::Vec3;
use openrw_rs::{
    CameraController, CameraMode, CameraTarget, CameraInput,
    PlayerStateMachine, PlayerState, PlayerContext,
    WeaponTargeting, Stance,
};

fn main() {
    // 1. Initialize Camera
    let mut camera = CameraController::new(CameraMode::default());
    let target = CameraTarget {
        position: Vec3::new(0.0, 1.0, 0.0),
        forward: Vec3::Z,
        ..Default::default()
    };
    let output = camera.update(0.016, &target, &CameraInput::default(), None::<&()>);

    // 2. Drive Player State Machine
    let mut sm = PlayerStateMachine::new(PlayerState::Idle);
    let ctx = PlayerContext {
        is_grounded: true,
        horizontal_speed: 6.0,
        ..Default::default()
    };
    sm.update(0.016, &ctx);
    assert_eq!(sm.current_state(), PlayerState::Run);

    // 3. Compute Weapon Spread & Ray
    let mut weapon = WeaponTargeting::default();
    let ray = weapon.fire_ray(
        output.position,
        output.forward(),
        Stance::Standing,
        &[],
        0.5,
        0.5,
    );
}
```

## Running Tests

```bash
cargo test
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
