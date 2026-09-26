use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Trait for querying environmental ray collisions to prevent camera clipping through walls/geometry.
pub trait RaycastWorld {
    /// Casts a ray from `origin` in `dir` (normalized) up to `max_dist`.
    /// Returns hit info if an obstacle was struck.
    fn cast_ray(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RaycastHit>;
}

/// Hit information returned by environment raycasting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RaycastHit {
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Collision clipping configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraCollisionConfig {
    /// Minimum distance to prevent the camera from inverting into the player.
    pub min_distance: f32,
    /// Clearance distance to keep away from hit geometry surfaces.
    pub clip_margin: f32,
    /// Sphere check radius around the camera lens.
    pub probe_radius: f32,
}

impl Default for CameraCollisionConfig {
    fn default() -> Self {
        Self {
            min_distance: 0.5,
            clip_margin: 0.25,
            probe_radius: 0.2,
        }
    }
}

/// Damped spring configuration for smooth camera follow.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraSpring {
    /// Higher values reach the target faster (e.g., 8.0 - 25.0).
    pub stiffness: f32,
    /// Damping factor: 1.0 is critically damped (no overshoot), < 1.0 underdamped, > 1.0 overdamped.
    pub damping: f32,
}

impl Default for CameraSpring {
    fn default() -> Self {
        Self {
            stiffness: 14.0,
            damping: 1.0,
        }
    }
}

impl CameraSpring {
    /// Critically damped smoothing step for 3D vector.
    pub fn smooth_step(&self, current: Vec3, target: Vec3, velocity: &mut Vec3, dt: f32) -> Vec3 {
        if dt <= 0.0 {
            return current;
        }
        let omega = self.stiffness;
        let zeta = self.damping;

        // Semi-implicit Euler spring damper
        let diff = current - target;
        let spring_acc = -omega * omega * diff - 2.0 * zeta * omega * *velocity;
        *velocity += spring_acc * dt;
        let new_pos = current + *velocity * dt;

        // Clamp to target if close and velocity is tiny
        if (new_pos - target).length_squared() < 1e-6 && velocity.length_squared() < 1e-4 {
            *velocity = Vec3::ZERO;
            target
        } else {
            new_pos
        }
    }
}

/// Information about the subject the camera is following (player or vehicle).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraTarget {
    /// World position of the target root (e.g. feet or vehicle center).
    pub position: Vec3,
    /// Height offset to player's head or vehicle pivot center.
    pub look_offset: Vec3,
    /// Forward heading vector (normalized in XZ plane).
    pub forward: Vec3,
    /// Up vector (usually Vec3::Y).
    pub up: Vec3,
    /// Current linear velocity of target in world space.
    pub velocity: Vec3,
    /// Whether target is currently in a vehicle.
    pub in_vehicle: bool,
}

impl Default for CameraTarget {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            look_offset: Vec3::new(0.0, 1.6, 0.0),
            forward: Vec3::Z,
            up: Vec3::Y,
            velocity: Vec3::ZERO,
            in_vehicle: false,
        }
    }
}

impl CameraTarget {
    pub fn focus_point(&self) -> Vec3 {
        self.position + self.look_offset
    }

    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }
}

/// User input affecting camera orientation.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CameraInput {
    /// Mouse / Right-stick delta X (yaw rotation).
    pub look_delta_x: f32,
    /// Mouse / Right-stick delta Y (pitch rotation).
    pub look_delta_y: f32,
    /// Zoom scroll input (+1 zoom in, -1 zoom out).
    pub zoom_delta: f32,
    /// Whether player is looking behind (bumper/rear mirror).
    pub look_back: bool,
}

/// Camera modes available in classic OpenRW / GTA III style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CameraMode {
    /// Behind-player third-person follow with spring smoothing and vehicle pull-back.
    BehindPlayerFollow {
        base_distance: f32,
        height: f32,
        pitch_angle: f32,
        lateral_offset: f32,
        /// Additional distance added per unit of speed.
        speed_zoom_factor: f32,
    },
    /// First-person or hood/bumper mount camera.
    Bumper {
        forward_offset: f32,
        height: f32,
        pitch: f32,
        yaw: f32,
    },
    /// Dramatic cinematic angles switching dynamically as in GTA III cinematic view.
    CinematicChase {
        angle_index: usize,
        timer: f32,
        interval: f32,
    },
    /// Freely rotatable mouse orbit camera with pitch and distance limits.
    Orbit {
        distance: f32,
        pitch: f32,
        yaw: f32,
        min_distance: f32,
        max_distance: f32,
        min_pitch: f32,
        max_pitch: f32,
    },
}

impl Default for CameraMode {
    fn default() -> Self {
        Self::BehindPlayerFollow {
            base_distance: 4.5,
            height: 1.8,
            pitch_angle: -0.15,
            lateral_offset: 0.0,
            speed_zoom_factor: 0.08,
        }
    }
}

/// Output computed by the camera system each frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraOutput {
    pub position: Vec3,
    pub rotation: Quat,
    pub look_at: Vec3,
    pub fov_degrees: f32,
}

impl CameraOutput {
    /// Generates standard 4x4 View Matrix (look-at matrix).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.look_at, Vec3::Y)
    }

    /// Forward direction vector camera is facing.
    pub fn forward(&self) -> Vec3 {
        (self.look_at - self.position).normalize_or_zero()
    }
}

/// Primary camera controller managing modes, springs, and collision raycasting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraController {
    pub mode: CameraMode,
    pub position: Vec3,
    pub look_at: Vec3,
    pub position_velocity: Vec3,
    pub look_at_velocity: Vec3,
    pub spring: CameraSpring,
    pub collision_config: CameraCollisionConfig,
    pub base_fov: f32,
    pub mouse_sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            mode: CameraMode::default(),
            position: Vec3::new(0.0, 2.0, -5.0),
            look_at: Vec3::ZERO,
            position_velocity: Vec3::ZERO,
            look_at_velocity: Vec3::ZERO,
            spring: CameraSpring::default(),
            collision_config: CameraCollisionConfig::default(),
            base_fov: 65.0,
            mouse_sensitivity: 0.003,
        }
    }
}

impl CameraController {
    pub fn new(mode: CameraMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    /// Hard resets camera directly to current target without spring lag.
    pub fn snap_to_target(&mut self, target: &CameraTarget) {
        let (desired_pos, desired_look) = self.calculate_ideal_pose(target, &CameraInput::default());
        self.position = desired_pos;
        self.look_at = desired_look;
        self.position_velocity = Vec3::ZERO;
        self.look_at_velocity = Vec3::ZERO;
    }

    /// Updates camera state for the frame with spring smoothing and raycast collision clipping.
    pub fn update(
        &mut self,
        dt: f32,
        target: &CameraTarget,
        input: &CameraInput,
        world: Option<&impl RaycastWorld>,
    ) -> CameraOutput {
        self.apply_input(dt, input);

        let (ideal_pos, ideal_look) = self.calculate_ideal_pose(target, input);

        // Smooth position and look-at using spring dampers
        let smoothed_pos = self
            .spring
            .smooth_step(self.position, ideal_pos, &mut self.position_velocity, dt);
        let smoothed_look = self
            .spring
            .smooth_step(self.look_at, ideal_look, &mut self.look_at_velocity, dt);

        let focus = target.focus_point();

        // Environment collision clipping: raycast from focus point to camera position
        let final_pos = if let Some(world) = world {
            self.resolve_collision(focus, smoothed_pos, world)
        } else {
            smoothed_pos
        };

        self.position = final_pos;
        self.look_at = smoothed_look;

        let forward = (self.look_at - self.position).normalize_or_zero();
        let rotation = if forward.length_squared() > 1e-4 {
            Quat::from_mat4(&Mat4::look_to_rh(self.position, forward, Vec3::Y))
        } else {
            Quat::IDENTITY
        };

        // Dynamic FOV widening at higher speeds
        let speed = target.speed();
        let dynamic_fov = self.base_fov + (speed * 0.35).min(20.0);

        CameraOutput {
            position: self.position,
            rotation,
            look_at: self.look_at,
            fov_degrees: dynamic_fov,
        }
    }

    /// Internal input update.
    fn apply_input(&mut self, dt: f32, input: &CameraInput) {
        match &mut self.mode {
            CameraMode::Orbit {
                pitch,
                yaw,
                distance,
                min_distance,
                max_distance,
                min_pitch,
                max_pitch,
            } => {
                *yaw -= input.look_delta_x * self.mouse_sensitivity;
                *pitch -= input.look_delta_y * self.mouse_sensitivity;
                *pitch = pitch.clamp(*min_pitch, *max_pitch);

                *distance -= input.zoom_delta * 0.8;
                *distance = distance.clamp(*min_distance, *max_distance);
            }
            CameraMode::CinematicChase {
                angle_index,
                timer,
                interval,
            } => {
                *timer += dt;
                if *timer >= *interval {
                    *timer = 0.0;
                    *angle_index = (*angle_index + 1) % 4;
                }
            }
            CameraMode::Bumper { pitch, yaw, .. } => {
                *yaw -= input.look_delta_x * self.mouse_sensitivity * 0.5;
                *pitch -= input.look_delta_y * self.mouse_sensitivity * 0.5;
                *pitch = pitch.clamp(-0.4, 0.4);
                *yaw = yaw.clamp(-1.0, 1.0);
            }
            CameraMode::BehindPlayerFollow { .. } => {
                // In behind player follow mode, look deltas can bias lateral offset or yaw slightly
            }
        }
    }

    /// Calculates ideal un-clipped target camera position and look-at point.
    pub fn calculate_ideal_pose(
        &self,
        target: &CameraTarget,
        input: &CameraInput,
    ) -> (Vec3, Vec3) {
        let focus = target.focus_point();
        let fwd = if target.forward.length_squared() > 1e-4 {
            target.forward.normalize()
        } else {
            Vec3::Z
        };
        let right = fwd.cross(target.up).normalize();

        match &self.mode {
            CameraMode::BehindPlayerFollow {
                base_distance,
                height,
                pitch_angle: _,
                lateral_offset,
                speed_zoom_factor,
            } => {
                let direction = if input.look_back { fwd } else { -fwd };
                let speed_dist = target.speed() * speed_zoom_factor;
                let total_dist = base_distance + speed_dist;

                let ideal_pos = focus
                    + direction * total_dist
                    + target.up * *height
                    + right * *lateral_offset;
                let ideal_look = focus;
                (ideal_pos, ideal_look)
            }
            CameraMode::Bumper {
                forward_offset,
                height,
                pitch,
                yaw,
            } => {
                let look_dir = if input.look_back {
                    -fwd
                } else {
                    let rot = Quat::from_axis_angle(target.up, *yaw)
                        * Quat::from_axis_angle(right, *pitch);
                    rot * fwd
                };

                let pos = focus + fwd * *forward_offset + target.up * *height;
                let look = pos + look_dir * 10.0;
                (pos, look)
            }
            CameraMode::CinematicChase { angle_index, .. } => {
                let speed = target.speed();
                let pull_back = 4.0 + speed * 0.1;
                let (offset_dir, height_mod) = match angle_index {
                    0 => (-fwd + right * 0.4, 1.2), // Classic low diagonal chase
                    1 => (right * 0.9 - fwd * 0.3, 0.6), // Side wheel / ground cam
                    2 => (-fwd * 1.5, 3.5),         // High dramatic overview
                    _ => (fwd * 0.8 + right * 0.3, 1.0), // Front angle oncoming
                };

                let pos = focus + offset_dir.normalize() * pull_back + target.up * height_mod;
                let look = focus;
                (pos, look)
            }
            CameraMode::Orbit {
                distance,
                pitch,
                yaw,
                ..
            } => {
                let cos_pitch = pitch.cos();
                let sin_pitch = pitch.sin();
                let cos_yaw = yaw.cos();
                let sin_yaw = yaw.sin();

                let offset = Vec3::new(
                    *distance * cos_pitch * sin_yaw,
                    *distance * sin_pitch,
                    *distance * cos_pitch * cos_yaw,
                );

                let pos = focus + offset;
                let look = focus;
                (pos, look)
            }
        }
    }

    /// Prevents camera from penetrating world geometry.
    fn resolve_collision(
        &self,
        focus: Vec3,
        desired_pos: Vec3,
        world: &impl RaycastWorld,
    ) -> Vec3 {
        let diff = desired_pos - focus;
        let dist = diff.length();
        if dist <= 1e-4 {
            return desired_pos;
        }

        let dir = diff / dist;
        if let Some(hit) = world.cast_ray(focus, dir, dist + self.collision_config.probe_radius) {
            let safe_dist = (hit.distance - self.collision_config.clip_margin)
                .clamp(self.collision_config.min_distance, dist);
            focus + dir * safe_dist
        } else {
            desired_pos
        }
    }
}
