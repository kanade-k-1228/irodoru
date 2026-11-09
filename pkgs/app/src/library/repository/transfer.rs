use super::*;

impl Library {
    /// Copy selected characters and their directly referenced audio, once per voice ID.
    /// Script IDs in generated voice metadata are historical provenance, not ownership.
    pub fn copy_characters_to(&self, target: &Library, ids: &[String]) -> Result<()> {
        let mut copied = std::collections::HashSet::new();
        for id in ids {
            let character = self.character(id)?;
            for voice in &character.voice_ids {
                if copied.insert(voice.clone()) {
                    target.store_voice(&self.voice_record(voice)?, &self.audio(voice)?)?;
                }
            }
            target.save_character(character.id.clone(), character.into())?;
        }
        Ok(())
    }

    /// Remap IDs when importing into an existing project; never replace existing audio.
    pub fn copy_character_input_to(
        &self,
        target: &Library,
        mut input: CharacterInput,
    ) -> Result<()> {
        // Stage and validate the complete bundle before changing the target project.
        let directory = tempfile::Builder::new()
            .prefix(".character-import-")
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
        let character = staged.create_character(input)?;
        let paths = ids
            .iter()
            .map(|id| PathBuf::from("voices").join(format!("{id}.wav")))
            .chain(std::iter::once(
                PathBuf::from("characters").join(character.id),
            ));
        let mut installed = Vec::new();
        for path in paths {
            let destination = target.root.join(&path);
            let result = (|| -> Result<()> {
                anyhow::ensure!(!destination.exists(), "コピー先の ID が重複しています");
                std::fs::rename(staged.root.join(&path), &destination)?;
                Ok(())
            })();
            if let Err(error) = result {
                for path in installed {
                    if std::fs::metadata(&path)?.is_dir() {
                        std::fs::remove_dir_all(path)?;
                    } else {
                        std::fs::remove_file(path)?;
                    }
                }
                return Err(error);
            }
            installed.push(destination);
        }
        Ok(())
    }
}
