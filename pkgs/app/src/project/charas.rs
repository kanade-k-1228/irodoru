//! Portable chara bundles use the same tables and archive validation as projects.
use super::{
    Session,
    archive::{extract, snapshot, write_archive},
};
use crate::domain::CharaInput;
use anyhow::{Result, ensure};
use std::{fs::File, path::Path};

impl Session {
    pub fn export_chara(&self, input: CharaInput, path: &Path) -> Result<()> {
        ensure!(
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("irochar")),
            "キャラクターファイルは .irochar で保存してください"
        );
        let bundle = Session::create(path.with_extension("iro"))?;
        self.library.copy_chara_input_to(&bundle.library, input)?;
        let mut snapshot = snapshot(&bundle)?;
        snapshot.path = path.to_owned();
        write_archive(&snapshot)
    }

    pub fn import_chara(&self, path: &Path) -> Result<()> {
        ensure!(
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("irochar")),
            "キャラクターファイルを選択してください"
        );
        let bundle = Session::create(path.with_extension("iro"))?;
        extract(File::open(path)?, bundle.library.root())?;
        bundle.validate()?;
        ensure!(
            bundle.library.editor()?.lines.is_empty(),
            "キャラクターファイルに台本が含まれています"
        );
        let charas = bundle.library.charas()?;
        ensure!(
            charas.len() == 1,
            "キャラクターファイルには1人のキャラクターが必要です"
        );
        let c = charas.into_iter().next().unwrap();
        bundle
            .library
            .copy_chara_input_to(&self.library, c.into())?;
        Ok(())
    }
}
