use super::Studio;
use crate::domain::AudioSource;
use crate::workspace::protocol::Command;

impl Studio {
    pub(super) fn play(&mut self, source: AudioSource) {
        if self.audio_id.as_ref() == Some(&source)
            && let Some(p) = &self.playback
            && p.position() < p.duration
        {
            p.toggle();
            return;
        }
        self.start_audio(source);
    }
    pub(super) fn start_audio(&mut self, source: AudioSource) {
        self.playback = None;
        self.audio_id = Some(source.clone());
        self.send(Command::Audio(source));
    }
}
