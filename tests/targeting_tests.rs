use glam::Vec3;
use openrw_rs::{
    AutoAimCandidate, AutoAimConfig, AutoAimSystem, BallisticSolver, BulletSpread, Stance,
    TargetingRay, WeaponTargeting,
};

#[test]
fn test_targeting_ray_point() {
    let ray = TargetingRay::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 100.0);
    let p = ray.point_at(25.0);
    assert_eq!(p, Vec3::new(0.0, 1.0, 25.0));

    // Clamped at max range
    let p_clamped = ray.point_at(150.0);
    assert_eq!(p_clamped, Vec3::new(0.0, 1.0, 100.0));
}

#[test]
fn test_stance_spread_modifiers() {
    let spread = BulletSpread::default();

    let crouch_spread = spread.effective_spread_angle(Stance::Crouching);
    let stand_spread = spread.effective_spread_angle(Stance::Standing);
    let run_spread = spread.effective_spread_angle(Stance::Running);

    assert!(crouch_spread < stand_spread);
    assert!(stand_spread < run_spread);
}

#[test]
fn test_recoil_bloom_and_recovery() {
    let mut spread = BulletSpread {
        base_spread_rad: 0.02,
        max_spread_rad: 0.10,
        bloom_per_shot_rad: 0.03,
        recovery_rate_rad: 0.05,
        current_bloom_rad: 0.0,
    };

    assert_eq!(spread.current_bloom_rad, 0.0);
    spread.on_shot_fired();
    assert_eq!(spread.current_bloom_rad, 0.03);

    spread.on_shot_fired();
    assert_eq!(spread.current_bloom_rad, 0.06);

    // Over-firing caps at max_spread - base_spread = 0.08
    spread.on_shot_fired();
    assert_eq!(spread.current_bloom_rad, 0.08);

    // Cool down for 1.0 sec
    spread.update(1.0);
    assert!((spread.current_bloom_rad - 0.03).abs() < 1e-4);

    // Cool down remainder
    spread.update(1.0);
    assert_eq!(spread.current_bloom_rad, 0.0);
}

#[test]
fn test_auto_aim_prioritization() {
    let mut system = AutoAimSystem::new(AutoAimConfig {
        max_range: 50.0,
        max_lock_angle_rad: 1.0,
        distance_weight: 1.0,
        angle_weight: 2.0,
        threat_bonus: 3.0,
    });

    let shooter_pos = Vec3::ZERO;
    let aim_dir = Vec3::Z;

    let candidates = vec![
        // Candidate 1: Close, directly ahead, innocent ped
        AutoAimCandidate {
            id: 1,
            position: Vec3::new(0.0, 0.0, 10.0),
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: false,
            is_alive: true,
            has_line_of_sight: true,
        },
        // Candidate 2: Slightly further, directly ahead, active threat with gun
        AutoAimCandidate {
            id: 2,
            position: Vec3::new(0.0, 0.0, 12.0),
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: true,
            is_alive: true,
            has_line_of_sight: true,
        },
        // Candidate 3: Obstructed by wall
        AutoAimCandidate {
            id: 3,
            position: Vec3::new(0.0, 0.0, 5.0),
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: true,
            is_alive: true,
            has_line_of_sight: false,
        },
        // Candidate 4: Dead body
        AutoAimCandidate {
            id: 4,
            position: Vec3::new(0.0, 0.0, 4.0),
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: true,
            is_alive: false,
            has_line_of_sight: true,
        },
    ];

    let best = system.select_best_target(shooter_pos, aim_dir, &candidates);
    assert!(best.is_some());
    // Candidate 2 has threat bonus that outweighs small distance difference
    assert_eq!(best.unwrap().id, 2);
}

#[test]
fn test_auto_aim_cycle_targets() {
    let mut system = AutoAimSystem::new(AutoAimConfig::default());
    let shooter_pos = Vec3::ZERO;
    let aim_dir = Vec3::Z;

    let candidates = vec![
        AutoAimCandidate {
            id: 10,
            position: Vec3::new(-4.0, 0.0, 10.0), // Left
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: false,
            is_alive: true,
            has_line_of_sight: true,
        },
        AutoAimCandidate {
            id: 20,
            position: Vec3::new(0.0, 0.0, 10.0), // Center
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: false,
            is_alive: true,
            has_line_of_sight: true,
        },
        AutoAimCandidate {
            id: 30,
            position: Vec3::new(4.0, 0.0, 10.0), // Right
            velocity: Vec3::ZERO,
            radius: 0.5,
            is_threat: false,
            is_alive: true,
            has_line_of_sight: true,
        },
    ];

    // Select center initially
    system.locked_target_id = Some(20);

    // Cycle right -> should pick id 30
    let right_target = system.cycle_target(shooter_pos, aim_dir, &candidates, true);
    assert_eq!(right_target.unwrap().id, 30);

    // Cycle right again -> wraps around to id 10
    let wrapped = system.cycle_target(shooter_pos, aim_dir, &candidates, true);
    assert_eq!(wrapped.unwrap().id, 10);
}

#[test]
fn test_ballistic_lead_interception() {
    let shooter = Vec3::ZERO;
    let target_pos = Vec3::new(30.0, 0.0, 40.0); // 50m away
    let target_vel = Vec3::new(10.0, 0.0, 0.0); // Moving +X at 10m/s
    let projectile_speed = 100.0; // 100 m/s

    let solution = BallisticSolver::solve_lead_target(shooter, target_pos, target_vel, projectile_speed);
    assert!(solution.is_some());

    let (aim_dir, flight_time) = solution.unwrap();
    assert!(flight_time > 0.0);

    let proj_pos_at_impact = shooter + aim_dir * projectile_speed * flight_time;
    let target_pos_at_impact = target_pos + target_vel * flight_time;

    assert!((proj_pos_at_impact - target_pos_at_impact).length() < 0.01);
}

#[test]
fn test_composite_weapon_targeting() {
    let mut weapon = WeaponTargeting::default();
    let candidates = vec![];
    let ray = weapon.fire_ray(
        Vec3::new(0.0, 1.5, 0.0),
        Vec3::Z,
        Stance::Standing,
        &candidates,
        0.5,
        0.5,
    );

    assert_eq!(ray.origin, Vec3::new(0.0, 1.5, 0.0));
    assert!(ray.direction.z > 0.9);
    assert!(weapon.spread.current_bloom_rad > 0.0);
}
