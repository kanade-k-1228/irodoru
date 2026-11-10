use super::*;

impl Library {
    /// Copy charas with independent reference audio in each chara directory.
    pub fn copy_charas_to(&self, target: &Library, ids: &[String]) -> Result<()> {
        for id in ids {
            let chara = self.chara(id)?;
            target.save_chara_from(chara.id.clone(), chara.into(), self)?;
        }
        Ok(())
    }

    /// Remap IDs when importing into an existing project; never replace existing audio.
    pub fn copy_chara_input_to(&self, target: &Library, mut input: CharaInput) -> Result<()> {
        // Stage and validate the complete bundle before changing the target project.
        let directory = tempfile::Builder::new()
            .prefix(".chara-import-")
            .tempdir_in(target.root())?;
        let staged = Library::open(directory.path())?;
        input = input.normalize(new_id())?.into();
        let mut ids = Vec::new();
        for source_id in &input.voice_ids {
            let mut voice = self.voice_record(source_id)?;
            let id = new_id();
            match &mut voice {
                Voice::Imported { id: stored, .. } => *stored = id.clone(),
                Voice::Generated(take) => take.id = id.clone(),
            }
            staged.store_voice(&voice, &self.audio(source_id)?)?;
            ids.push(id);
        }
        input.voice_ids = ids.clone();
        let chara = staged.create_chara(input)?;
        let path = PathBuf::from("charas").join(chara.id);
        let destination = target.root.join(&path);
        anyhow::ensure!(!destination.exists(), "コピー先の ID が重複しています");
        std::fs::rename(staged.root.join(path), destination)?;
        Ok(())
    }
}
