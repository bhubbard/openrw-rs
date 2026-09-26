//! # openrw-rs
//!
//! Pure Rust port of core GTA-style gameplay systems inspired by OpenRW (rwengine):
//! - **Camera**: Third-person follow with spring smoothing, bumper, cinematic chase, mouse orbit, and collision clipping.
//! - **Targeting**: Direction raycasting, auto-aim lock-on prioritization, bullet spread cone, and ballistic projectile trajectory calculation.
//! - **Player State**: ECS-compatible player state machine with validated transitions and timers.

pub mod camera;
pub mod player_state;
pub mod targeting;

pub use camera::{
    CameraCollisionConfig, CameraController, CameraInput, CameraMode, CameraOutput, CameraSpring,
    CameraTarget, RaycastHit, RaycastWorld,
};
pub use player_state::{
    PlayerContext, PlayerState, PlayerStateMachine, StateTransitionEvent, TransitionError,
    VehicleSeat,
};
pub use targeting::{
    AutoAimCandidate, AutoAimConfig, AutoAimSystem, BallisticSolver, BulletSpread, Stance,
    TargetingRay, WeaponTargeting,
};
