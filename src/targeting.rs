use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Basic targeting ray used for hitscan weapons and line-of-sight checks.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TargetingRay {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_range: f32,
}

impl TargetingRay {
    pub fn new(origin: Vec3, direction: Vec3, max_range: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
            max_range,
        }
    }

    /// Computes point along ray at distance `t`.
    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t.clamp(0.0, self.max_range)
    }
}

/// Stance of the shooter affecting spread and recoil stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Stance {
    #[default]
    Standing,
    Crouching,
    Walking,
    Running,
    InVehicle,
}

impl Stance {
    /// Multiplier applied to base weapon spread angle.
    pub fn spread_multiplier(&self) -> f32 {
        match self {
            Stance::Crouching => 0.65, // Crouching increases accuracy
            Stance::Standing => 1.0,
            Stance::Walking => 1.35,
            Stance::Running => 1.85,
            Stance::InVehicle => 1.5,
        }
    }
}

/// Bullet spread and cone-of-fire manager.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BulletSpread {
    /// Minimum spread angle in radians (perfect aim base).
    pub base_spread_rad: f32,
    /// Maximum spread angle in radians during continuous fire bloom.
    pub max_spread_rad: f32,
    /// Spread angle increase per shot fired.
    pub bloom_per_shot_rad: f32,
    /// Speed at which bloom cools down per second (rad/sec).
    pub recovery_rate_rad: f32,
    /// Current accumulated heat / bloom angle.
    pub current_bloom_rad: f32,
}

impl Default for BulletSpread {
    fn default() -> Self {
        Self {
            base_spread_rad: 0.015,       // ~0.86 degrees base
            max_spread_rad: 0.12,         // ~6.8 degrees max bloom
            bloom_per_shot_rad: 0.02,     // bloom kick per shot
            recovery_rate_rad: 0.08,      // recovers over ~1.5s
            current_bloom_rad: 0.0,
        }
    }
}

impl BulletSpread {
    /// Call when a shot is fired to add recoil bloom.
    pub fn on_shot_fired(&mut self) {
        self.current_bloom_rad = (self.current_bloom_rad + self.bloom_per_shot_rad)
            .min(self.max_spread_rad - self.base_spread_rad);
    }

    /// Decay bloom over frame delta time.
    pub fn update(&mut self, dt: f32) {
        if dt > 0.0 {
            self.current_bloom_rad = (self.current_bloom_rad - self.recovery_rate_rad * dt).max(0.0);
        }
    }

    /// Effective half-angle of the cone of fire given current stance.
    pub fn effective_spread_angle(&self, stance: Stance) -> f32 {
        let raw = self.base_spread_rad + self.current_bloom_rad;
        (raw * stance.spread_multiplier()).min(self.max_spread_rad)
    }

    /// Perturbs a normalized aim direction by applying random spread inside the cone.
    /// `u1` and `u2` are uniform random floats in [0.0, 1.0).
    pub fn apply_spread(&self, aim_dir: Vec3, stance: Stance, u1: f32, u2: f32) -> Vec3 {
        let max_angle = self.effective_spread_angle(stance);
        if max_angle <= 1e-5 {
            return aim_dir.normalize_or_zero();
        }

        // Uniform disk sampling mapped to spherical cone
        let r = u1.sqrt() * max_angle;
        let theta = u2 * std::f32::consts::TAU;

        let local_x = r * theta.cos();
        let local_y = r * theta.sin();

        // Build an orthonormal basis with Z aligned with aim_dir
        let z = aim_dir.normalize_or_zero();
        let up = if z.y.abs() < 0.999 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let x = up.cross(z).normalize();
        let y = z.cross(x).normalize();

        (z + x * local_x + y * local_y).normalize()
    }
}

/// Target candidate for GTA-style auto-aim lock-on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoAimCandidate {
    pub id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    /// Bounding sphere radius or hit radius.
    pub radius: f32,
    /// Whether target is an active hostile / threat (higher lock priority).
    pub is_threat: bool,
    /// Whether target is alive and valid.
    pub is_alive: bool,
    /// Direct line of sight to target.
    pub has_line_of_sight: bool,
}

/// Auto-aim evaluation configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoAimConfig {
    /// Maximum range to engage lock-on.
    pub max_range: f32,
    /// Maximum angle from aim direction to consider (in radians).
    pub max_lock_angle_rad: f32,
    /// Scoring weight for closeness (1.0 = equal, higher prefers close).
    pub distance_weight: f32,
    /// Scoring weight for angle alignment (higher prefers directly centered targets).
    pub angle_weight: f32,
    /// Scoring bonus for active threats (cops, armed gangsters).
    pub threat_bonus: f32,
}

impl Default for AutoAimConfig {
    fn default() -> Self {
        Self {
            max_range: 45.0,
            max_lock_angle_rad: 55.0_f32.to_radians(),
            distance_weight: 1.0,
            angle_weight: 2.2,
            threat_bonus: 2.0,
        }
    }
}

/// System for scoring and selecting targets.
#[derive(Debug, Clone, Default)]
pub struct AutoAimSystem {
    pub config: AutoAimConfig,
    pub locked_target_id: Option<u64>,
}

impl AutoAimSystem {
    pub fn new(config: AutoAimConfig) -> Self {
        Self {
            config,
            locked_target_id: None,
        }
    }

    /// Evaluates candidate targets and selects the highest scoring target.
    pub fn select_best_target<'a>(
        &mut self,
        shooter_pos: Vec3,
        aim_direction: Vec3,
        candidates: &'a [AutoAimCandidate],
    ) -> Option<&'a AutoAimCandidate> {
        let aim_norm = aim_direction.normalize_or_zero();
        let mut best_target: Option<&'a AutoAimCandidate> = None;
        let mut best_score = f32::NEG_INFINITY;

        for c in candidates {
            if !c.is_alive || !c.has_line_of_sight {
                continue;
            }

            if let Some(score) = self.score_candidate(shooter_pos, aim_norm, c) {
                if score > best_score {
                    best_score = score;
                    best_target = Some(c);
                }
            }
        }

        self.locked_target_id = best_target.map(|t| t.id);
        best_target
    }

    /// Cycle to the next target to the left or right of current lock-on.
    pub fn cycle_target<'a>(
        &mut self,
        shooter_pos: Vec3,
        aim_direction: Vec3,
        candidates: &'a [AutoAimCandidate],
        cycle_right: bool,
    ) -> Option<&'a AutoAimCandidate> {
        let aim_norm = aim_direction.normalize_or_zero();
        let up = Vec3::Y;
        let right = up.cross(aim_norm).normalize_or_zero();

        let mut valid_candidates: Vec<(&AutoAimCandidate, f32)> = candidates
            .iter()
            .filter(|c| c.is_alive && c.has_line_of_sight)
            .filter_map(|c| {
                let to_target = c.position - shooter_pos;
                let dist = to_target.length();
                if dist > self.config.max_range || dist <= 1e-4 {
                    return None;
                }
                let dir = to_target / dist;
                let lateral = dir.dot(right);
                Some((c, lateral))
            })
            .collect();

        if valid_candidates.is_empty() {
            return None;
        }

        // Sort by lateral offset (left to right)
        valid_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let current_index = self
            .locked_target_id
            .and_then(|id| valid_candidates.iter().position(|(c, _)| c.id == id));

        let next_idx = match current_index {
            Some(idx) => {
                if cycle_right {
                    (idx + 1) % valid_candidates.len()
                } else if idx == 0 {
                    valid_candidates.len() - 1
                } else {
                    idx - 1
                }
            }
            None => 0,
        };

        let selected = valid_candidates[next_idx].0;
        self.locked_target_id = Some(selected.id);
        Some(selected)
    }

    /// Computes match score for a candidate. Higher is better.
    pub fn score_candidate(
        &self,
        shooter_pos: Vec3,
        aim_direction: Vec3,
        candidate: &AutoAimCandidate,
    ) -> Option<f32> {
        let to_target = candidate.position - shooter_pos;
        let distance = to_target.length();

        if distance > self.config.max_range || distance <= 1e-4 {
            return None;
        }

        let dir = to_target / distance;
        let cos_angle = aim_direction.dot(dir).clamp(-1.0, 1.0);
        let angle = cos_angle.acos();

        if angle > self.config.max_lock_angle_rad {
            return None;
        }

        // Distance factor: 1.0 at 0m down to 0.0 at max_range
        let dist_factor = 1.0 - (distance / self.config.max_range);

        // Alignment factor: 1.0 directly in front down to 0.0 at cone limit
        let angle_factor = 1.0 - (angle / self.config.max_lock_angle_rad);

        let threat_factor = if candidate.is_threat {
            self.config.threat_bonus
        } else {
            0.0
        };

        let score = (dist_factor * self.config.distance_weight)
            + (angle_factor * self.config.angle_weight)
            + threat_factor;

        Some(score)
    }
}

/// Ballistics and projectile physics calculations (rockets, grenades, mortars, sniper drop).
pub struct BallisticSolver;

impl BallisticSolver {
    /// Computes position along ballistic arc at elapsed flight time `t`.
    pub fn sample_trajectory(
        origin: Vec3,
        initial_velocity: Vec3,
        gravity: Vec3,
        t: f32,
    ) -> Vec3 {
        origin + initial_velocity * t + 0.5 * gravity * t * t
    }

    /// Computes velocity along ballistic arc at elapsed flight time `t`.
    pub fn sample_velocity(initial_velocity: Vec3, gravity: Vec3, t: f32) -> Vec3 {
        initial_velocity + gravity * t
    }

    /// Generates polyline points approximating flight path.
    pub fn generate_arc_points(
        origin: Vec3,
        initial_velocity: Vec3,
        gravity: Vec3,
        max_time: f32,
        step: f32,
    ) -> Vec<Vec3> {
        let mut points = Vec::new();
        let mut t = 0.0;
        while t <= max_time {
            points.push(Self::sample_trajectory(origin, initial_velocity, gravity, t));
            t += step;
        }
        points
    }

    /// Interception targeting calculation for leading a moving target with constant projectile speed.
    ///
    /// Given:
    /// - shooter_pos: position of shooter
    /// - target_pos: current position of target
    /// - target_vel: linear velocity of target
    /// - projectile_speed: muzzle speed of projectile (scalar)
    ///
    /// Returns:
    /// - `Some((intercept_dir, time_of_flight))` if target can be intercepted, or `None` if out of reach.
    pub fn solve_lead_target(
        shooter_pos: Vec3,
        target_pos: Vec3,
        target_vel: Vec3,
        projectile_speed: f32,
    ) -> Option<(Vec3, f32)> {
        if projectile_speed <= 1e-4 {
            return None;
        }

        let r = target_pos - shooter_pos;
        let v = target_vel;
        let s = projectile_speed;

        // Quadratic equation: |r + v*t|^2 = (s*t)^2
        // => (v.dot(v) - s^2)*t^2 + 2*(r.dot(v))*t + r.dot(r) = 0
        let a = v.length_squared() - s * s;
        let b = 2.0 * r.dot(v);
        let c = r.length_squared();

        if a.abs() < 1e-6 {
            // Linear case when target speed equals projectile speed
            if b.abs() < 1e-6 {
                return None;
            }
            let t = -c / b;
            if t > 0.0 {
                let predicted = target_pos + v * t;
                let dir = (predicted - shooter_pos).normalize_or_zero();
                return Some((dir, t));
            }
            return None;
        }

        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None; // No real solution: projectile cannot catch target
        }

        let sqrt_disc = discriminant.sqrt();
        let t1 = (-b - sqrt_disc) / (2.0 * a);
        let t2 = (-b + sqrt_disc) / (2.0 * a);

        // Find smallest positive time
        let valid_time = match (t1 > 0.0, t2 > 0.0) {
            (true, true) => Some(t1.min(t2)),
            (true, false) => Some(t1),
            (false, true) => Some(t2),
            (false, false) => None,
        }?;

        let predicted_hit_point = target_pos + v * valid_time;
        let fire_dir = (predicted_hit_point - shooter_pos).normalize_or_zero();

        Some((fire_dir, valid_time))
    }
}

/// Composite weapon targeting manager combining raycasting, spread, and auto-aim.
#[derive(Debug, Clone, Default)]
pub struct WeaponTargeting {
    pub spread: BulletSpread,
    pub auto_aim: AutoAimSystem,
}

impl WeaponTargeting {
    pub fn new(spread: BulletSpread, auto_aim: AutoAimSystem) -> Self {
        Self { spread, auto_aim }
    }

    /// Computes final fired ray taking into account auto-aim lock-on and stance spread.
    pub fn fire_ray(
        &mut self,
        muzzle_pos: Vec3,
        view_dir: Vec3,
        stance: Stance,
        candidates: &[AutoAimCandidate],
        u1: f32,
        u2: f32,
    ) -> TargetingRay {
        let base_dir = if let Some(target) = self.auto_aim.select_best_target(muzzle_pos, view_dir, candidates) {
            (target.position - muzzle_pos).normalize_or_zero()
        } else {
            view_dir.normalize_or_zero()
        };

        let perturbed_dir = self.spread.apply_spread(base_dir, stance, u1, u2);
        self.spread.on_shot_fired();

        TargetingRay::new(muzzle_pos, perturbed_dir, 150.0)
    }
}
