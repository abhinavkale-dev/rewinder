use crate::core::state::{LifecycleState, PermissionStateDto};

pub fn boot_state(permission: &PermissionStateDto, replay_enabled: bool) -> LifecycleState {
    if !replay_enabled {
        return LifecycleState::Disabled;
    }

    if permission.screen_recording_granted
        && permission.system_audio_granted
        && permission.output_dir_writable
    {
        LifecycleState::Armed
    } else {
        LifecycleState::PermissionRequired
    }
}

pub fn idle_state(permission: &PermissionStateDto, replay_enabled: bool) -> LifecycleState {
    boot_state(permission, replay_enabled)
}
