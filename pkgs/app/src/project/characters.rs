//! Portable character bundles use the same tables and archive validation as projects.
use super::{
    Session,
    archive::{extract, snapshot, write_archive},
};
use crate::domain::CharacterInput;
use anyhow::{Result, ensure};
use std::{fs::File, path::Path};

impl Session {
    pub fn export_character(&self, input: CharacterInput, path: &Path) -> Result<()> {
        ensure!(
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("irochar")),
            "キャラクターファイルは .irochar で保存してください"
        );
        let bundle = Session::create(path.with_extension("iro"))?;
        self.library
            .copy_character_input_to(&bundle.library, input)?;
        let mut snapshot = snapshot(&bundle)?;
        snapshot.path = path.to_owned();
        write_archive(&snapshot)
    }

    pub fn import_character(&self, path: &Path) -> Result<()> {
        ensure!(
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("irochar")),
            "キャラクターファイルを選択してください"
        );
        let bundle = Session::create(path.with_extension("iro"))?;
        extract(File::open(path)?, bundle.library.root())?;
        bundle.validate()?;
        ensure!(
            bundle.library.editor()?.rows.is_empty(),
            "キャラクターファイルに台本が含まれています"
        );
        let characters = bundle.library.characters()?;
        ensure!(
            characters.len() == 1,
            "キャラクターファイルには1人のキャラクターが必要です"
        );
        let c = characters.into_iter().next().unwrap();
        bundle
            .library
            .copy_character_input_to(&self.library, c.into())?;
        Ok(())
    }
}
