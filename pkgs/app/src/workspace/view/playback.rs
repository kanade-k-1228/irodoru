use super::Studio;
use crate::workspace::protocol::Command;

impl Studio {
    pub(super) fn play(&mut self, id: String, preview: bool) {
        let key = format!("{preview}:{id}");
        if self.audio_id.as_ref() == Some(&key)
            && let Some(p) = &self.playback
            && p.position() < p.duration
        {
            p.toggle();
            return;
        }
        self.playback = None;
        self.audio_id = Some(key);
        self.send(Command::Audio { id, preview }, true);
    }
}
