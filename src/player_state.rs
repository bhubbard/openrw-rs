use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Car seat assigned when entering or occupying a vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VehicleSeat {
    #[default]
    Driver,
    PassengerFront,
    PassengerRearLeft,
    PassengerRearRight,
}

/// Core player state enum representing locomotion, vehicle, combat, and physics states.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PlayerState {
    Idle,
    Walk,
    Run,
    Sprint {
        stamina: f32,
    },
    EnterVehicle {
        vehicle_id: u32,
        seat: VehicleSeat,
        timer: f32,
        duration: f32,
    },
    InVehicle {
        vehicle_id: u32,
        seat: VehicleSeat,
    },
    ExitVehicle {
        vehicle_id: u32,
        timer: f32,
        duration: f32,
        is_bailout: bool,
    },
    Aim {
        weapon_id: u32,
    },
    Shoot {
        weapon_id: u32,
        timer: f32,
        recovery: f32,
    },
    Jump {
        timer: f32,
        min_air_time: f32,
    },
    Fall {
        air_time: f32,
        fall_start_y: f32,
    },
    Land {
        timer: f32,
        recovery_time: f32,
        is_hard_landing: bool,
    },
    Ragdoll {
        timer: f32,
        duration: f32,
        impact_speed: f32,
    },
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState::Idle
    }
}

/// Transition errors returned when attempting an illegal state switch.
#[derive(Debug, Error, PartialEq, Clone, Serialize, Deserialize)]
pub enum TransitionError {
    #[error("Cannot transition to {target:?} while in mid-air/falling")]
    AirborneRestriction { target: String },
    #[error("Cannot enter vehicle while already in a vehicle or entering/exiting")]
    VehicleOccupied,
    #[error("Cannot perform action while immobilized in ragdoll (remaining: {remaining:.2}s)")]
    RagdollImmobilized { remaining: f32 },
    #[error("Cannot transition from {current:?} to {target:?}: invalid state path")]
    InvalidTransition { current: String, target: String },
    #[error("Insufficient stamina for sprint")]
    InsufficientStamina,
    #[error("Landing recovery active (remaining: {remaining:.2}s)")]
    LandingLocked { remaining: f32 },
    #[error("Vehicle transition animation in progress")]
    VehicleAnimationActive,
}

/// External context fed into the state machine on each tick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlayerContext {
    pub is_grounded: bool,
    pub current_altitude_y: f32,
    pub vertical_velocity: f32,
    pub horizontal_speed: f32,
    pub stamina: f32,
    pub wants_to_sprint: bool,
    pub wants_to_aim: bool,
    pub wants_to_shoot: bool,
    pub wants_to_jump: bool,
    pub active_weapon_id: Option<u32>,
}

impl Default for PlayerContext {
    fn default() -> Self {
        Self {
            is_grounded: true,
            current_altitude_y: 0.0,
            vertical_velocity: 0.0,
            horizontal_speed: 0.0,
            stamina: 100.0,
            wants_to_sprint: false,
            wants_to_aim: false,
            wants_to_shoot: false,
            wants_to_jump: false,
            active_weapon_id: None,
        }
    }
}

/// Transition event emitted when state changes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StateTransitionEvent {
    pub from: PlayerState,
    pub to: PlayerState,
}

/// Main player state machine managing validation, timer updates, and transitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PlayerStateMachine {
    current_state: PlayerState,
}

impl PlayerStateMachine {
    pub fn new(initial: PlayerState) -> Self {
        Self {
            current_state: initial,
        }
    }

    pub fn current_state(&self) -> PlayerState {
        self.current_state
    }

    /// Validates if transition from current state to `next` is allowed.
    pub fn validate_transition(
        &self,
        current: &PlayerState,
        next: &PlayerState,
    ) -> Result<(), TransitionError> {
        let name_curr = format!("{:?}", current);
        let name_next = format!("{:?}", next);

        // Ragdoll cannot transition unless duration expired
        if let PlayerState::Ragdoll {
            timer, duration, ..
        } = current
        {
            if *timer < *duration && !matches!(next, PlayerState::Ragdoll { .. }) {
                return Err(TransitionError::RagdollImmobilized {
                    remaining: duration - timer,
                });
            }
        }

        // Hard landing locks movement until recovery expires
        if let PlayerState::Land {
            timer,
            recovery_time,
            is_hard_landing,
        } = current
        {
            if *is_hard_landing && *timer < *recovery_time && !matches!(next, PlayerState::Ragdoll { .. }) {
                return Err(TransitionError::LandingLocked {
                    remaining: recovery_time - timer,
                });
            }
        }

        // Entering/exiting vehicle animation lock
        if let PlayerState::EnterVehicle {
            timer, duration, ..
        }
        | PlayerState::ExitVehicle {
            timer, duration, ..
        } = current
        {
            if *timer < *duration && !matches!(next, PlayerState::Ragdoll { .. } | PlayerState::InVehicle { .. } | PlayerState::Idle) {
                return Err(TransitionError::VehicleAnimationActive);
            }
        }

        // Disallow entering vehicle if already in one
        if matches!(current, PlayerState::InVehicle { .. }) && matches!(next, PlayerState::EnterVehicle { .. }) {
            return Err(TransitionError::VehicleOccupied);
        }

        // Ragdoll can be triggered from virtually any state (impact/explosion)
        if matches!(next, PlayerState::Ragdoll { .. }) {
            return Ok(());
        }

        match current {
            PlayerState::Idle | PlayerState::Walk | PlayerState::Run => match next {
                PlayerState::Idle
                | PlayerState::Walk
                | PlayerState::Run
                | PlayerState::Sprint { .. }
                | PlayerState::Jump { .. }
                | PlayerState::Fall { .. }
                | PlayerState::Aim { .. }
                | PlayerState::Shoot { .. }
                | PlayerState::EnterVehicle { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::Sprint { .. } => match next {
                PlayerState::Idle
                | PlayerState::Walk
                | PlayerState::Run
                | PlayerState::Jump { .. }
                | PlayerState::Fall { .. }
                | PlayerState::Aim { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::Jump { .. } => match next {
                PlayerState::Fall { .. } | PlayerState::Land { .. } => Ok(()),
                _ => Err(TransitionError::AirborneRestriction {
                    target: name_next,
                }),
            },
            PlayerState::Fall { .. } => match next {
                PlayerState::Land { .. } => Ok(()),
                _ => Err(TransitionError::AirborneRestriction {
                    target: name_next,
                }),
            },
            PlayerState::Land { .. } => match next {
                PlayerState::Idle
                | PlayerState::Walk
                | PlayerState::Run
                | PlayerState::Sprint { .. }
                | PlayerState::Aim { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::Aim { .. } => match next {
                PlayerState::Idle
                | PlayerState::Walk
                | PlayerState::Run
                | PlayerState::Shoot { .. }
                | PlayerState::Fall { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::Shoot { .. } => match next {
                PlayerState::Aim { .. }
                | PlayerState::Idle
                | PlayerState::Walk
                | PlayerState::Run
                | PlayerState::Fall { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::EnterVehicle { .. } => match next {
                PlayerState::InVehicle { .. } | PlayerState::Idle => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::InVehicle { .. } => match next {
                PlayerState::ExitVehicle { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::ExitVehicle { .. } => match next {
                PlayerState::Idle | PlayerState::Walk | PlayerState::Run => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
            PlayerState::Ragdoll { .. } => match next {
                PlayerState::Idle | PlayerState::Land { .. } => Ok(()),
                _ => Err(TransitionError::InvalidTransition {
                    current: name_curr,
                    target: name_next,
                }),
            },
        }
    }

    /// Directly requests a state change.
    pub fn transition_to(&mut self, next: PlayerState) -> Result<StateTransitionEvent, TransitionError> {
        self.validate_transition(&self.current_state, &next)?;
        let prev = self.current_state;
        self.current_state = next;
        Ok(StateTransitionEvent {
            from: prev,
            to: next,
        })
    }

    /// Ticks state timers and automatic physics transitions.
    pub fn update(
        &mut self,
        dt: f32,
        ctx: &PlayerContext,
    ) -> Option<StateTransitionEvent> {
        let mut event: Option<StateTransitionEvent> = None;

        match &mut self.current_state {
            PlayerState::Jump { timer, min_air_time } => {
                *timer += dt;
                // Transition to Fall if apex reached or moving downward
                if *timer >= *min_air_time && ctx.vertical_velocity < 0.0 {
                    let next = PlayerState::Fall {
                        air_time: *timer,
                        fall_start_y: ctx.current_altitude_y,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Fall { air_time, fall_start_y } => {
                *air_time += dt;
                if ctx.is_grounded {
                    let fall_distance = (*fall_start_y - ctx.current_altitude_y).max(0.0);
                    let is_hard = fall_distance > 6.0 || ctx.vertical_velocity.abs() > 14.0;
                    let recovery = if is_hard { 1.2 } else { 0.2 };
                    let next = PlayerState::Land {
                        timer: 0.0,
                        recovery_time: recovery,
                        is_hard_landing: is_hard,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Land {
                timer,
                recovery_time,
                is_hard_landing: _,
            } => {
                *timer += dt;
                if *timer >= *recovery_time {
                    let next = if ctx.horizontal_speed > 4.5 {
                        PlayerState::Run
                    } else if ctx.horizontal_speed > 0.5 {
                        PlayerState::Walk
                    } else {
                        PlayerState::Idle
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Ragdoll { timer, duration, .. } => {
                *timer += dt;
                if *timer >= *duration && ctx.is_grounded {
                    let next = PlayerState::Idle;
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::EnterVehicle {
                vehicle_id,
                seat,
                timer,
                duration,
            } => {
                *timer += dt;
                if *timer >= *duration {
                    let next = PlayerState::InVehicle {
                        vehicle_id: *vehicle_id,
                        seat: *seat,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::ExitVehicle {
                timer,
                duration,
                is_bailout,
                ..
            } => {
                *timer += dt;
                if *timer >= *duration {
                    let next = if *is_bailout {
                        PlayerState::Ragdoll {
                            timer: 0.0,
                            duration: 2.5,
                            impact_speed: ctx.horizontal_speed,
                        }
                    } else {
                        PlayerState::Idle
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Shoot {
                weapon_id,
                timer,
                recovery,
            } => {
                *timer += dt;
                if *timer >= *recovery {
                    let next = if ctx.wants_to_aim {
                        PlayerState::Aim {
                            weapon_id: *weapon_id,
                        }
                    } else {
                        PlayerState::Idle
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Sprint { stamina } => {
                if !ctx.wants_to_sprint || ctx.horizontal_speed <= 3.5 || *stamina <= 0.0 {
                    let next = if ctx.horizontal_speed > 0.5 {
                        PlayerState::Run
                    } else {
                        PlayerState::Idle
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Idle | PlayerState::Walk | PlayerState::Run => {
                if !ctx.is_grounded && ctx.vertical_velocity < -1.0 {
                    let next = PlayerState::Fall {
                        air_time: 0.0,
                        fall_start_y: ctx.current_altitude_y,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.wants_to_jump && ctx.is_grounded {
                    let next = PlayerState::Jump {
                        timer: 0.0,
                        min_air_time: 0.2,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.wants_to_shoot && ctx.active_weapon_id.is_some() {
                    let next = PlayerState::Shoot {
                        weapon_id: ctx.active_weapon_id.unwrap(),
                        timer: 0.0,
                        recovery: 0.25,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.wants_to_aim && ctx.active_weapon_id.is_some() {
                    let next = PlayerState::Aim {
                        weapon_id: ctx.active_weapon_id.unwrap(),
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.wants_to_sprint && ctx.horizontal_speed > 3.5 && ctx.stamina > 10.0 {
                    let next = PlayerState::Sprint {
                        stamina: ctx.stamina,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.horizontal_speed > 4.5 && !matches!(self.current_state, PlayerState::Run) {
                    let next = PlayerState::Run;
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.horizontal_speed > 0.5
                    && ctx.horizontal_speed <= 4.5
                    && !matches!(self.current_state, PlayerState::Walk)
                {
                    let next = PlayerState::Walk;
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if ctx.horizontal_speed <= 0.5 && !matches!(self.current_state, PlayerState::Idle) {
                    let next = PlayerState::Idle;
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::Aim { weapon_id } => {
                if ctx.wants_to_shoot {
                    let next = PlayerState::Shoot {
                        weapon_id: *weapon_id,
                        timer: 0.0,
                        recovery: 0.25,
                    };
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                } else if !ctx.wants_to_aim {
                    let next = PlayerState::Idle;
                    event = Some(StateTransitionEvent {
                        from: self.current_state,
                        to: next,
                    });
                    self.current_state = next;
                }
            }
            PlayerState::InVehicle { .. } => {
                // Maintained until an explicit exit or bailout trigger occurs
            }
        }

        event
    }
}
