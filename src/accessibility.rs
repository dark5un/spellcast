// SPDX-License-Identifier: Apache-2.0

//! Accessibility and UX polish.
//!
//! Audio feedback tones, screen reader events, onboarding wizard.

use std::io::{self, Write};

/// Audio feedback for mode transitions and events.
pub struct AudioFeedback {
    /// Whether audio feedback is enabled.
    pub enabled: bool,
}

impl AudioFeedback {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Play a short ascending tone (enter dictation mode).
    pub fn enter_dictation(&self) {
        if !self.enabled {
            return;
        }
        self.beep(440, 100); // A4, 100ms
        self.beep(660, 100); // E5, 100ms
    }

    /// Play a short descending tone (exit dictation mode).
    pub fn exit_dictation(&self) {
        if !self.enabled {
            return;
        }
        self.beep(660, 100);
        self.beep(440, 100);
    }

    /// Play a distinct alert tone (kill switch activated).
    pub fn kill_switch(&self) {
        if !self.enabled {
            return;
        }
        for _ in 0..3 {
            self.beep(880, 80);
        }
    }

    /// Play a soft chime (explain feature complete).
    pub fn explain_complete(&self) {
        if !self.enabled {
            return;
        }
        self.beep(523, 150); // C5
        self.beep(659, 150); // E5
        self.beep(784, 200); // G5
    }

    fn beep(&self, _freq: u32, duration_ms: u32) {
        // Write BEL character for terminal bell (simplest cross-platform approach)
        let _ = io::stdout().write_all(b"\x07");
        let _ = io::stdout().flush();
        // In production, use cpal or a dedicated tone generator
        std::thread::sleep(std::time::Duration::from_millis(duration_ms as u64));
    }
}

/// Screen reader event emission (AT-SPI on Linux).
pub struct ScreenReaderEvents;

impl ScreenReaderEvents {
    /// Announce a mode change to the screen reader.
    pub fn announce_mode(mode: &str) {
        Self::speak(&format!("Spellcast mode: {}", mode));
    }

    /// Announce the current token.
    pub fn announce_token(token: &str) {
        Self::speak(&format!("Token: {}", token));
    }

    /// Speak text using the terminal bell or speech-dispatcher.
    fn speak(text: &str) {
        // Try speech-dispatcher first (spd-say)
        let _ = std::process::Command::new("spd-say")
            .args(["-t", "female3", text])
            .output();
        // Fallback: write to stderr (Orca reads from terminal output)
        let _ = writeln!(io::stderr(), "{}", text);
    }
}

/// Onboarding wizard state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    Welcome,
    MicrophoneTest,
    GpuDetection,
    ModelDownload,
    DictationTest,
    KeyBindings,
    KillSwitch,
    Complete,
}

impl OnboardingStep {
    pub fn next(&self) -> OnboardingStep {
        match self {
            OnboardingStep::Welcome => OnboardingStep::MicrophoneTest,
            OnboardingStep::MicrophoneTest => OnboardingStep::GpuDetection,
            OnboardingStep::GpuDetection => OnboardingStep::ModelDownload,
            OnboardingStep::ModelDownload => OnboardingStep::DictationTest,
            OnboardingStep::DictationTest => OnboardingStep::KeyBindings,
            OnboardingStep::KeyBindings => OnboardingStep::KillSwitch,
            OnboardingStep::KillSwitch => OnboardingStep::Complete,
            OnboardingStep::Complete => OnboardingStep::Complete,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            OnboardingStep::Welcome => {
                "Welcome to Spellcast. I'll guide you through setup.\nPress Enter to start."
            }
            OnboardingStep::MicrophoneTest => {
                "Step 1: Testing your microphone.\nPlease say 'hello world' into your microphone.\nListening..."
            }
            OnboardingStep::GpuDetection => {
                "Step 2: Detecting your GPU.\nSpellcast will auto-detect the best compute backend."
            }
            OnboardingStep::ModelDownload => {
                "Step 3: Downloading the ASR model.\nThis downloads Whisper base.en (~150MB)."
            }
            OnboardingStep::DictationTest => {
                "Step 4: Dictation test.\nSay 'hello world' and see it appear as text."
            }
            OnboardingStep::KeyBindings => {
                "Step 5: Key bindings.\nCaps Lock toggles dictation. H/L navigate tokens.\nCtrl+\\ is the kill switch."
            }
            OnboardingStep::KillSwitch => {
                "Step 6: Test the kill switch.\nPress Ctrl+\\ now to confirm it works."
            }
            OnboardingStep::Complete => {
                "Setup complete! Spellcast is ready.\nYou can start dictating by pressing Caps Lock."
            }
        }
    }
}

pub struct OnboardingWizard {
    pub current_step: OnboardingStep,
    pub microphone_ok: bool,
    pub gpu_ok: bool,
    pub model_ok: bool,
}

impl Default for OnboardingWizard {
    fn default() -> Self {
        Self::new()
    }
}

impl OnboardingWizard {
    pub fn new() -> Self {
        Self {
            current_step: OnboardingStep::Welcome,
            microphone_ok: false,
            gpu_ok: false,
            model_ok: false,
        }
    }

    /// Advance to the next step.
    pub fn advance(&mut self) {
        self.current_step = self.current_step.next();
    }

    /// Mark microphone test as passed.
    pub fn mark_microphone_ok(&mut self) {
        self.microphone_ok = true;
    }

    /// Mark GPU detection as passed.
    pub fn mark_gpu_ok(&mut self) {
        self.gpu_ok = true;
    }

    /// Mark model download as complete.
    pub fn mark_model_ok(&mut self) {
        self.model_ok = true;
    }

    pub fn is_complete(&self) -> bool {
        self.current_step == OnboardingStep::Complete
    }

    /// Render the current step as a status string.
    pub fn status_string(&self) -> String {
        let mut s = format!("\n{}\n\n", self.current_step.description());
        s.push_str(&format!(
            "Microphone: {}\n",
            if self.microphone_ok { "✓" } else { "○" }
        ));
        s.push_str(&format!(
            "GPU:       {}\n",
            if self.gpu_ok { "✓" } else { "○" }
        ));
        s.push_str(&format!(
            "Model:     {}\n",
            if self.model_ok { "✓" } else { "○" }
        ));
        s.push_str("\nPress Enter to continue, Esc to skip.");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_steps() {
        let mut wiz = OnboardingWizard::new();
        assert_eq!(wiz.current_step, OnboardingStep::Welcome);
        assert!(!wiz.is_complete());

        wiz.advance();
        assert_eq!(wiz.current_step, OnboardingStep::MicrophoneTest);

        // Skip through all steps
        while !wiz.is_complete() {
            wiz.advance();
        }
        assert!(wiz.is_complete());
    }

    #[test]
    fn test_onboarding_status() {
        let wiz = OnboardingWizard::new();
        let status = wiz.status_string();
        assert!(status.contains("Microphone"));
        assert!(status.contains("GPU"));
        assert!(status.contains("Model"));
    }

    #[test]
    fn test_mark_complete() {
        let mut wiz = OnboardingWizard::new();
        wiz.mark_microphone_ok();
        wiz.mark_gpu_ok();
        wiz.mark_model_ok();
        assert!(wiz.microphone_ok);
        assert!(wiz.gpu_ok);
        assert!(wiz.model_ok);
    }

    #[test]
    fn test_audio_feedback_creates_without_panic() {
        let fb = AudioFeedback::new(false);
        fb.enter_dictation();
        fb.exit_dictation();
        fb.kill_switch();
        fb.explain_complete();
        // If we got here, no panic
        assert!(true);
    }

    #[test]
    fn test_screen_reader_announce() {
        // Should not panic
        ScreenReaderEvents::announce_mode("dictation");
        ScreenReaderEvents::announce_token("hello");
        assert!(true);
    }

    #[test]
    fn test_onboarding_step_next_wraps() {
        assert_eq!(OnboardingStep::Complete.next(), OnboardingStep::Complete);
    }

    #[test]
    fn test_step_descriptions_not_empty() {
        for step in &[
            OnboardingStep::Welcome,
            OnboardingStep::MicrophoneTest,
            OnboardingStep::GpuDetection,
            OnboardingStep::ModelDownload,
            OnboardingStep::DictationTest,
            OnboardingStep::KeyBindings,
            OnboardingStep::KillSwitch,
            OnboardingStep::Complete,
        ] {
            assert!(!step.description().is_empty());
        }
    }
}
