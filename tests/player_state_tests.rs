use openrw_rs::{
    PlayerContext, PlayerState, PlayerStateMachine, TransitionError, VehicleSeat,
};

#[test]
fn test_locomotion_transitions() {
    let mut sm = PlayerStateMachine::new(PlayerState::Idle);

    assert_eq!(sm.current_state(), PlayerState::Idle);
    assert!(sm.transition_to(PlayerState::Walk).is_ok());
    assert_eq!(sm.current_state(), PlayerState::Walk);

    assert!(sm.transition_to(PlayerState::Run).is_ok());
    assert_eq!(sm.current_state(), PlayerState::Run);

    assert!(sm.transition_to(PlayerState::Sprint { stamina: 100.0 }).is_ok());
    assert!(matches!(sm.current_state(), PlayerState::Sprint { .. }));
}

#[test]
fn test_airborne_restrictions() {
    let mut sm = PlayerStateMachine::new(PlayerState::Jump {
        timer: 0.1,
        min_air_time: 0.2,
    });

    // Cannot transition to Walk or Sprint while in midair
    let err = sm.transition_to(PlayerState::Walk);
    assert!(matches!(err, Err(TransitionError::AirborneRestriction { .. })));

    // Transitioning to Fall is allowed
    assert!(sm
        .transition_to(PlayerState::Fall {
            air_time: 0.2,
            fall_start_y: 5.0,
        })
        .is_ok());
}

#[test]
fn test_fall_to_landing_recovery() {
    let mut sm = PlayerStateMachine::new(PlayerState::Fall {
        air_time: 1.0,
        fall_start_y: 12.0,
    });

    let ctx = PlayerContext {
        is_grounded: true,
        current_altitude_y: 0.0,
        vertical_velocity: -16.0, // High speed impact => hard landing
        ..Default::default()
    };

    // Updating when grounded should trigger Land state
    let ev = sm.update(0.016, &ctx);
    assert!(ev.is_some());
    assert!(matches!(
        sm.current_state(),
        PlayerState::Land {
            is_hard_landing: true,
            ..
        }
    ));

    // While in hard landing recovery, attempts to walk should fail
    let walk_res = sm.transition_to(PlayerState::Walk);
    assert!(matches!(walk_res, Err(TransitionError::LandingLocked { .. })));

    // Update through recovery period (1.2s)
    for _ in 0..100 {
        sm.update(0.016, &ctx);
    }

    // Now recovered to Idle or Walk based on horizontal speed
    assert_eq!(sm.current_state(), PlayerState::Idle);
}

#[test]
fn test_vehicle_enter_and_bailout() {
    let mut sm = PlayerStateMachine::new(PlayerState::Idle);

    // Start entering vehicle
    assert!(sm
        .transition_to(PlayerState::EnterVehicle {
            vehicle_id: 42,
            seat: VehicleSeat::Driver,
            timer: 0.0,
            duration: 1.5,
        })
        .is_ok());

    let ctx = PlayerContext::default();

    // Advance 1.6s
    sm.update(1.6, &ctx);
    assert_eq!(
        sm.current_state(),
        PlayerState::InVehicle {
            vehicle_id: 42,
            seat: VehicleSeat::Driver,
        }
    );

    // Bailout while moving fast
    let high_speed_ctx = PlayerContext {
        horizontal_speed: 25.0,
        ..Default::default()
    };
    assert!(sm
        .transition_to(PlayerState::ExitVehicle {
            vehicle_id: 42,
            timer: 0.0,
            duration: 0.5,
            is_bailout: true,
        })
        .is_ok());

    // Exit vehicle animation ends, triggers Ragdoll due to bailout
    sm.update(0.6, &high_speed_ctx);
    assert!(matches!(sm.current_state(), PlayerState::Ragdoll { .. }));
}

#[test]
fn test_ragdoll_immobilization() {
    let mut sm = PlayerStateMachine::new(PlayerState::Ragdoll {
        timer: 0.0,
        duration: 3.0,
        impact_speed: 20.0,
    });

    let ctx = PlayerContext {
        is_grounded: true,
        ..Default::default()
    };

    // Cannot transition to Walk while ragdolling
    let res = sm.transition_to(PlayerState::Walk);
    assert!(matches!(res, Err(TransitionError::RagdollImmobilized { .. })));

    // Advance timer past duration
    sm.update(3.1, &ctx);
    assert_eq!(sm.current_state(), PlayerState::Idle);
}

#[test]
fn test_aim_and_shoot_loop() {
    let mut sm = PlayerStateMachine::new(PlayerState::Idle);

    let mut ctx = PlayerContext {
        active_weapon_id: Some(7),
        wants_to_aim: true,
        ..Default::default()
    };

    sm.update(0.016, &ctx);
    assert_eq!(sm.current_state(), PlayerState::Aim { weapon_id: 7 });

    ctx.wants_to_shoot = true;
    sm.update(0.016, &ctx);
    assert!(matches!(sm.current_state(), PlayerState::Shoot { weapon_id: 7, .. }));

    // Finish shooting recoil recovery while still aiming
    ctx.wants_to_shoot = false;
    sm.update(0.3, &ctx);
    assert_eq!(sm.current_state(), PlayerState::Aim { weapon_id: 7 });

    // Stop aiming
    ctx.wants_to_aim = false;
    sm.update(0.016, &ctx);
    assert_eq!(sm.current_state(), PlayerState::Idle);
}
