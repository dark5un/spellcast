// SPDX-License-Identifier: Apache-2.0

//! Mode controller — manages the Spellcast mode state machine.
//!
//! Modes:
//! - **Dictation** (entered via Ctrl+Space or Caps Lock): Speech becomes text.
//!   Navigation/editing keys operate on tokens.
//! - **Raw** (default): Spellcast is transparent, all keys pass through.
//! - **Killed**: Kill switch engaged (Ctrl+G), Spellcast fully disabled until toggled again.
//!
//! Note: The mode toggle is Ctrl+Space (universal) or Caps Lock (kitty keyboard protocol).
//! The kill switch is Ctrl+G (detected as both Ctrl+G and BEL 0x07 in raw mode).

use serde::{Deserialize, Serialize};

/// Spellcast operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Dictation mode: speech → text, token navigation active
    Dictation,
    /// Raw passthrough: all keystrokes pass through transparently
    Raw,
    /// Killed: Spellcast fully disabled, hard panic mode
    Killed,
}

impl Mode {
    /// Returns true if Spellcast should process input.
    pub fn is_active(self) -> bool {
        matches!(self, Mode::Dictation)
    }

    /// Returns true if Spellcast is in raw passthrough.
    pub fn is_raw(self) -> bool {
        matches!(self, Mode::Raw)
    }

    /// Returns true if the kill switch is active.
    pub fn is_killed(self) -> bool {
        matches!(self, Mode::Killed)
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Dictation => write!(f, "DICT"),
            Mode::Raw => write!(f, "RAW"),
            Mode::Killed => write!(f, "KILLED"),
        }
    }
}

/// Controller for Spellcast mode transitions.
///
/// Handles:
/// - Ctrl+Space / Caps Lock toggle (Dictation ↔ Raw)
/// - Shift+Caps Lock for actual capital letters
/// - Kill switch (Ctrl+G) for hard disable/re-enable
#[derive(Debug, Clone)]
pub struct ModeController {
    mode: Mode,
    caps_lock_state: bool,
    kill_switch_engaged: bool,
}

impl ModeController {
    /// Create a new ModeController starting in Raw mode.
    pub fn new() -> Self {
        Self {
            mode: Mode::Raw,
            caps_lock_state: false,
            kill_switch_engaged: false,
        }
    }

    /// Create a new ModeController starting in the given mode.
    pub fn with_mode(mode: Mode) -> Self {
        Self {
            mode,
            caps_lock_state: false,
            kill_switch_engaged: false,
        }
    }

    /// Get the current mode.
    pub fn current_mode(&self) -> Mode {
        self.mode
    }

    /// Toggle between Dictation and Raw mode.
    /// Called when Caps Lock is pressed alone (not shifted).
    pub fn toggle_mode(&mut self) -> Mode {
        if self.kill_switch_engaged {
            return self.mode;
        }
        self.mode = match self.mode {
            Mode::Dictation => Mode::Raw,
            Mode::Raw => Mode::Dictation,
            Mode::Killed => Mode::Killed,
        };
        self.caps_lock_state = self.mode == Mode::Dictation;
        self.mode
    }

    /// Handle Shift+Caps Lock — toggles actual caps lock state.
    /// Returns the new caps lock state.
    pub fn toggle_caps_lock(&mut self) -> bool {
        self.caps_lock_state = !self.caps_lock_state;
        self.caps_lock_state
    }

    /// Engage the kill switch — forces Killed mode, Spellcast becomes transparent.
    pub fn engage_kill_switch(&mut self) -> Mode {
        self.mode = Mode::Killed;
        self.kill_switch_engaged = true;
        self.mode
    }

    /// Disengage the kill switch — returns to Raw mode.
    pub fn disengage_kill_switch(&mut self) -> Mode {
        self.mode = Mode::Raw;
        self.kill_switch_engaged = false;
        self.mode
    }

    /// Toggle the kill switch.
    pub fn toggle_kill_switch(&mut self) -> Mode {
        if self.kill_switch_engaged {
            self.disengage_kill_switch()
        } else {
            self.engage_kill_switch()
        }
    }

    /// Check if caps lock is active.
    pub fn is_caps_lock_active(&self) -> bool {
        self.caps_lock_state
    }
}

impl Default for ModeController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_mode_is_raw() {
        let ctrl = ModeController::new();
        assert_eq!(ctrl.current_mode(), Mode::Raw);
    }

    #[test]
    fn test_toggle_mode() {
        let mut ctrl = ModeController::new();
        assert_eq!(ctrl.toggle_mode(), Mode::Dictation);
        assert_eq!(ctrl.toggle_mode(), Mode::Raw);
    }

    #[test]
    fn test_toggle_caps_lock() {
        let mut ctrl = ModeController::new();
        assert!(!ctrl.is_caps_lock_active());
        assert!(ctrl.toggle_caps_lock());
        assert!(ctrl.is_caps_lock_active());
        assert!(!ctrl.toggle_caps_lock());
        assert!(!ctrl.is_caps_lock_active());
    }

    #[test]
    fn test_kill_switch() {
        let mut ctrl = ModeController::new();
        assert_eq!(ctrl.engage_kill_switch(), Mode::Killed);
        assert!(ctrl.current_mode().is_killed());
        assert_eq!(ctrl.disengage_kill_switch(), Mode::Raw);
        assert!(ctrl.current_mode().is_raw());
    }

    #[test]
    fn test_kill_switch_toggle() {
        let mut ctrl = ModeController::new();
        assert_eq!(ctrl.toggle_kill_switch(), Mode::Killed);
        assert_eq!(ctrl.toggle_kill_switch(), Mode::Raw);
    }

    #[test]
    fn test_kill_switch_prevents_mode_toggle() {
        let mut ctrl = ModeController::new();
        ctrl.engage_kill_switch();
        assert_eq!(ctrl.toggle_mode(), Mode::Killed);
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Dictation.to_string(), "DICT");
        assert_eq!(Mode::Raw.to_string(), "RAW");
        assert_eq!(Mode::Killed.to_string(), "KILLED");
    }

    #[test]
    fn test_mode_queries() {
        assert!(Mode::Dictation.is_active());
        assert!(!Mode::Dictation.is_raw());
        assert!(!Mode::Dictation.is_killed());

        assert!(!Mode::Raw.is_active());
        assert!(Mode::Raw.is_raw());

        assert!(!Mode::Killed.is_active());
        assert!(Mode::Killed.is_killed());
    }

    #[test]
    fn test_with_mode() {
        let ctrl = ModeController::with_mode(Mode::Dictation);
        assert_eq!(ctrl.current_mode(), Mode::Dictation);
    }
}
