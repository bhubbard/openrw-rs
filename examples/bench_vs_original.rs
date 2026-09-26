use std::time::Instant;
use glam::Vec3;
use openrw_rs::{
    AutoAimCandidate, AutoAimConfig, AutoAimSystem, BallisticSolver, CameraController, CameraInput,
    CameraMode, CameraTarget, PlayerContext, PlayerState, PlayerStateMachine, RaycastWorld,
};

fn main() {
    println!("============================================================");
    println!("     openrw-rs (Rust) vs C++ OpenRW / RenderWare Bench      ");
    println!("============================================================");

    // 1. Third-Person Follow Camera Smoothing & Pose Step
    println!("\n--- 1. Third-Person Camera Step (Spring Smoothing & Bumper) ---");
    {
        let mut controller = CameraController::new(CameraMode::BehindPlayerFollow {
            base_distance: 5.0,
            height: 2.0,
            pitch_angle: 0.05,
            lateral_offset: 0.0,
            speed_zoom_factor: 0.15,
        });

        let target = CameraTarget {
            position: Vec3::new(10.0, 1.0, 10.0),
            forward: Vec3::new(0.0, 0.0, 1.0),
            velocity: Vec3::new(0.0, 0.0, 12.0),
            ..Default::default()
        };

        let input = CameraInput {
            look_delta_x: 0.01,
            look_delta_y: 0.005,
            zoom_delta: 0.0,
            look_back: false,
        };

        let iterations = 2_000_000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _ = controller.update(0.0166, &target, &input, None::<&MockWorld>);
        }

        let elapsed = start.elapsed();
        let ns_per_step = elapsed.as_nanos() as f64 / iterations as f64;
        let steps_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!(
            "Camera Updates: {} | Time: {:.2?} | Latency: {:.2} ns/step | {:>10.0} updates/s",
            iterations, elapsed, ns_per_step, steps_per_sec
        );
    }

    // 2. Auto-Aim Lock-On Prioritization (Scoring 100 Candidates)
    println!("\n--- 2. Auto-Aim Prioritization (100 Active Pedestrian Targets) ---");
    {
        let mut auto_aim = AutoAimSystem::new(AutoAimConfig::default());
        let player_pos = Vec3::new(0.0, 1.0, 0.0);
        let aim_forward = Vec3::new(0.0, 0.0, 1.0);

        let candidates: Vec<AutoAimCandidate> = (0..100)
            .map(|i| AutoAimCandidate {
                id: i,
                position: Vec3::new(
                    (i % 10) as f32 * 2.0 - 10.0,
                    1.0,
                    (i / 10) as f32 * 5.0 + 5.0,
                ),
                velocity: Vec3::ZERO,
                is_threat: i % 3 == 0,
                is_alive: true,
                has_line_of_sight: true,
                radius: 0.5,
            })
            .collect();

        let iterations = 200_000;
        let start = Instant::now();
        let mut total_locked = 0;

        for _ in 0..iterations {
            if let Some(target) = auto_aim.select_best_target(player_pos, aim_forward, &candidates) {
                total_locked += target.id;
            }
        }

        let elapsed = start.elapsed();
        let us_per_eval = elapsed.as_micros() as f64 / iterations as f64;
        let evals_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!(
            "Auto-Aim Queries: {} | Time: {:.2?} | Latency: {:.2} µs/query | {:>10.0} queries/s | Locked: {}",
            iterations, elapsed, us_per_eval, evals_per_sec, total_locked
        );
    }

    // 3. Ballistic Solver Projectile Arc Trajectory
    println!("\n--- 3. Ballistic Solver Sample Trajectory Points ---");
    {
        let origin = Vec3::new(0.0, 1.5, 0.0);
        let vel = Vec3::new(15.0, 20.0, 45.0);
        let gravity = Vec3::new(0.0, -9.81, 0.0);

        let iterations = 10_000_000;
        let start = Instant::now();
        let mut sum_y = 0.0;

        for i in 0..iterations {
            let t = (i % 100) as f32 * 0.02;
            let pt = BallisticSolver::sample_trajectory(origin, vel, gravity, t);
            sum_y += pt.y;
        }

        let elapsed = start.elapsed();
        let ns_per_solve = elapsed.as_nanos() as f64 / iterations as f64;
        let solves_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!(
            "Trajectory Samples: {} | Time: {:.2?} | Latency: {:.2} ns/sample ({:>10.0} samples/s) | SumY: {:.1}",
            iterations, elapsed, ns_per_solve, solves_per_sec, sum_y
        );
    }

    // 4. Player State Machine Transitions
    println!("\n--- 4. Player State Machine Validated Transitions ---");
    {
        let mut sm = PlayerStateMachine::new(PlayerState::Idle);
        let ctx = PlayerContext {
            horizontal_speed: 4.5,
            ..Default::default()
        };

        let iterations = 5_000_000;
        let start = Instant::now();
        let mut transition_count = 0;

        for i in 0..iterations {
            let next_state = match i % 3 {
                0 => PlayerState::Walk,
                1 => PlayerState::Run,
                _ => PlayerState::Idle,
            };
            if sm.transition_to(next_state).is_ok() {
                transition_count += 1;
            }
            let _ = sm.update(0.0166, &ctx);
        }

        let elapsed = start.elapsed();
        let ns_per_trans = elapsed.as_nanos() as f64 / iterations as f64;
        let trans_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!(
            "State Transitions: {} | Time: {:.2?} | Latency: {:.2} ns/event ({:>10.0} events/s) | Ok: {}",
            iterations, elapsed, ns_per_trans, trans_per_sec, transition_count
        );
    }

    println!("\n============================================================");
    println!("                      Benchmark Complete                    ");
    println!("============================================================");
}

struct MockWorld;
impl RaycastWorld for MockWorld {
    fn cast_ray(&self, _origin: Vec3, _dir: Vec3, _max_dist: f32) -> Option<openrw_rs::RaycastHit> {
        None
    }
}
