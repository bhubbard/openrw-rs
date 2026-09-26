use glam::Vec3;
use openrw_rs::{
    CameraCollisionConfig, CameraController, CameraInput, CameraMode, CameraSpring, CameraTarget,
    RaycastHit, RaycastWorld,
};

struct MockObstacleWorld {
    hit_distance: f32,
}

impl RaycastWorld for MockObstacleWorld {
    fn cast_ray(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RaycastHit> {
        if max_dist >= self.hit_distance {
            Some(RaycastHit {
                distance: self.hit_distance,
                point: origin + dir * self.hit_distance,
                normal: -dir,
            })
        } else {
            None
        }
    }
}

#[test]
fn test_behind_player_follow_positioning() {
    let mut controller = CameraController::new(CameraMode::BehindPlayerFollow {
        base_distance: 5.0,
        height: 2.0,
        pitch_angle: 0.0,
        lateral_offset: 0.0,
        speed_zoom_factor: 0.1,
    });

    let target = CameraTarget {
        position: Vec3::new(10.0, 0.0, 10.0),
        forward: Vec3::new(0.0, 0.0, 1.0),
        ..Default::default()
    };

    controller.snap_to_target(&target);

    // Behind camera should be at target.position - forward * 5.0 + up * 2.0 (with look_offset 1.6 => height 3.6)
    assert!((controller.position.x - 10.0).abs() < 1e-3);
    assert!((controller.position.z - 5.0).abs() < 1e-3);
    assert!((controller.position.y - 3.6).abs() < 1e-3);
}

#[test]
fn test_speed_zoom_expansion() {
    let controller = CameraController::new(CameraMode::BehindPlayerFollow {
        base_distance: 5.0,
        height: 2.0,
        pitch_angle: 0.0,
        lateral_offset: 0.0,
        speed_zoom_factor: 0.2,
    });

    let input = CameraInput::default();
    let stationary_target = CameraTarget {
        position: Vec3::ZERO,
        forward: Vec3::Z,
        velocity: Vec3::ZERO,
        ..Default::default()
    };
    let high_speed_target = CameraTarget {
        position: Vec3::ZERO,
        forward: Vec3::Z,
        velocity: Vec3::new(0.0, 0.0, 30.0), // 30 m/s
        ..Default::default()
    };

    let (pos_stat, _) = controller.calculate_ideal_pose(&stationary_target, &input);
    let (pos_fast, _) = controller.calculate_ideal_pose(&high_speed_target, &input);

    let dist_stat = (pos_stat - stationary_target.focus_point()).length();
    let dist_fast = (pos_fast - high_speed_target.focus_point()).length();

    assert!(dist_fast > dist_stat);
    // At 30 m/s, extra distance is 30 * 0.2 = 6.0m
    assert!((dist_fast - dist_stat - 5.5).abs() < 1.0);
}

#[test]
fn test_look_back_reversal() {
    let controller = CameraController::new(CameraMode::BehindPlayerFollow {
        base_distance: 4.0,
        height: 1.5,
        pitch_angle: 0.0,
        lateral_offset: 0.0,
        speed_zoom_factor: 0.0,
    });

    let normal_input = CameraInput {
        look_back: false,
        ..Default::default()
    };
    let look_back_input = CameraInput {
        look_back: true,
        ..Default::default()
    };

    let target = CameraTarget {
        position: Vec3::ZERO,
        forward: Vec3::new(0.0, 0.0, 1.0),
        ..Default::default()
    };

    let (pos_norm, _) = controller.calculate_ideal_pose(&target, &normal_input);
    let (pos_back, _) = controller.calculate_ideal_pose(&target, &look_back_input);

    // Normal should be in negative Z (behind target)
    assert!(pos_norm.z < 0.0);
    // Look-back should be in positive Z (in front of target)
    assert!(pos_back.z > 0.0);
}

#[test]
fn test_spring_smoothing() {
    let spring = CameraSpring {
        stiffness: 10.0,
        damping: 1.0,
    };
    let mut vel = Vec3::ZERO;
    let start = Vec3::ZERO;
    let target = Vec3::new(10.0, 0.0, 0.0);

    let step1 = spring.smooth_step(start, target, &mut vel, 0.05);
    assert!(step1.x > 0.0 && step1.x < 10.0);
    assert!(vel.x > 0.0);

    // Over multiple steps, it converges toward target
    let mut curr = step1;
    for _ in 0..60 {
        curr = spring.smooth_step(curr, target, &mut vel, 0.05);
    }
    assert!((curr.x - 10.0).abs() < 0.05);
}

#[test]
fn test_orbit_camera_controls() {
    let mut controller = CameraController::new(CameraMode::Orbit {
        distance: 5.0,
        pitch: 0.0,
        yaw: 0.0,
        min_distance: 2.0,
        max_distance: 10.0,
        min_pitch: -1.0,
        max_pitch: 1.0,
    });
    controller.mouse_sensitivity = 0.01;

    let target = CameraTarget::default();
    let input = CameraInput {
        look_delta_x: 50.0,
        look_delta_y: 20.0,
        zoom_delta: 2.0,
        look_back: false,
    };

    controller.update(0.016, &target, &input, None::<&MockObstacleWorld>);

    if let CameraMode::Orbit {
        pitch,
        yaw,
        distance,
        ..
    } = controller.mode
    {
        assert!(yaw < 0.0);
        assert!(pitch < 0.0);
        assert!(distance < 5.0); // Zoom in reduces distance
    } else {
        panic!("Wrong camera mode");
    }
}

#[test]
fn test_collision_clipping_wall_detection() {
    let mut controller = CameraController::new(CameraMode::BehindPlayerFollow {
        base_distance: 6.0,
        height: 0.0,
        pitch_angle: 0.0,
        lateral_offset: 0.0,
        speed_zoom_factor: 0.0,
    });
    controller.collision_config = CameraCollisionConfig {
        min_distance: 0.5,
        clip_margin: 0.3,
        probe_radius: 0.1,
    };

    let target = CameraTarget {
        position: Vec3::ZERO,
        look_offset: Vec3::ZERO,
        forward: Vec3::Z,
        ..Default::default()
    };
    controller.snap_to_target(&target);

    // Unobstructed ideal position would be at (0, 0, -6)
    // Wall intercepts at 2.5 meters
    let world = MockObstacleWorld { hit_distance: 2.5 };
    let input = CameraInput::default();
    let output = controller.update(0.016, &target, &input, Some(&world));

    let dist_from_focus = (output.position - target.focus_point()).length();
    // Expected distance: 2.5 - 0.3 clip_margin = 2.2
    assert!((dist_from_focus - 2.2).abs() < 0.1);
}

#[test]
fn test_cinematic_chase_cycle() {
    let mut controller = CameraController::new(CameraMode::CinematicChase {
        angle_index: 0,
        timer: 0.0,
        interval: 3.0,
    });

    let target = CameraTarget::default();
    let input = CameraInput::default();

    // Advance 3.5 seconds
    controller.update(3.5, &target, &input, None::<&MockObstacleWorld>);

    if let CameraMode::CinematicChase { angle_index, .. } = controller.mode {
        assert_eq!(angle_index, 1);
    } else {
        panic!("Expected CinematicChase mode");
    }
}
