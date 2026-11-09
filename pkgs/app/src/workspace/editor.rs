//! Editing rules shared by the native views and tests.
use crate::{
    domain::{Character, new_id},
    domain::{EditorState, GenerationSettings, ScriptRow, SynthesisInput, TakeRequest},
};
use anyhow::{Result, bail};

pub fn new_row(character_id: String, settings: GenerationSettings) -> ScriptRow {
    ScriptRow {
        id: new_id(),
        text: String::new(),
        character_id,
        prompt: None,
        settings: Some(settings),
        take_id: None,
    }
}
pub fn insert(editor: &mut EditorState, selected: Option<&str>, duplicate: bool) -> String {
    let index = selected.and_then(|id| editor.rows.iter().position(|r| r.id == id));
    let source = index.map(|i| &editor.rows[i]);
    let mut row = new_row(
        source
            .map(|r| r.character_id.clone())
            .unwrap_or_else(|| editor.active_character.clone()),
        source
            .and_then(|r| r.settings.clone())
            .unwrap_or_else(|| editor.preferences.clone()),
    );
    if duplicate && let Some(source) = source {
        row.text = source.text.clone();
        row.prompt = source.prompt.clone();
    }
    let id = row.id.clone();
    editor
        .rows
        .insert(index.map_or(editor.rows.len(), |i| i + 1), row);
    id
}
/// Move a row after another row, or to the beginning when `after` is absent.
pub fn move_row(rows: &mut Vec<ScriptRow>, id: &str, after: Option<&str>) -> Option<usize> {
    let index = rows.iter().position(|row| row.id == id)?;
    let slot = match after {
        Some(after) => rows.iter().position(|row| row.id == after)? + 1,
        None => 0,
    };
    if slot == index || slot == index + 1 {
        return None;
    }
    let target = slot - usize::from(index < slot);
    let row = rows.remove(index);
    rows.insert(target, row);
    Some(target)
}
pub fn remove(editor: &mut EditorState, id: &str) -> Option<String> {
    if editor.rows.len() <= 1 {
        return None;
    }
    let index = editor.rows.iter().position(|r| r.id == id)?;
    editor.rows.remove(index);
    Some(editor.rows[index.min(editor.rows.len() - 1)].id.clone())
}
pub fn speech(text: String, settings: &GenerationSettings) -> Result<SynthesisInput> {
    let seed = if settings.seed.trim().is_empty() {
        None
    } else {
        let seed = settings
            .seed
            .trim()
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("シードには0以上の安全な整数を指定してください"))?;
        if seed > 9_007_199_254_740_991 {
            bail!("シードには0以上の安全な整数を指定してください");
        }
        Some(seed)
    };
    let num_steps = if settings.num_steps.trim().is_empty() {
        None
    } else {
        let n = settings.num_steps.trim().parse::<usize>().unwrap_or(0);
        if !(1..=200).contains(&n) {
            bail!("ステップ数は1〜200で指定してください");
        }
        Some(n)
    };
    if !settings.duration_scale.is_finite()
        || !(0.5..=2.0).contains(&settings.duration_scale)
        || [
            settings.cfg_text,
            settings.cfg_speaker,
            settings.cfg_caption,
        ]
        .iter()
        .any(|n| !n.is_finite() || !(0.0..=10.0).contains(n))
    {
        bail!("生成設定が範囲外です");
    }
    Ok(SynthesisInput {
        text,
        seed,
        num_steps,
        duration_scale: settings.duration_scale,
        cfg_scale_text: settings.cfg_text,
        cfg_scale_speaker: settings.cfg_speaker,
        cfg_scale_caption: settings.cfg_caption,
    })
}
pub fn requests(
    rows: &[ScriptRow],
    characters: &[Character],
    defaults: &GenerationSettings,
) -> Result<Vec<TakeRequest>> {
    rows.iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| {
            if !characters.iter().any(|c| c.id == r.character_id) {
                bail!("各セリフのプリセットを選択してください");
            }
            Ok(TakeRequest {
                row_id: r.id.clone(),
                character_id: r.character_id.clone(),
                additional_prompt: r
                    .prompt
                    .as_deref()
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned),
                request: speech(r.text.clone(), r.settings.as_ref().unwrap_or(defaults))?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_has_new_identity_and_no_audio() {
        let mut e = EditorState::default();
        let first = insert(&mut e, None, false);
        e.rows[0].text = "こんにちは".into();
        e.rows[0].take_id = Some("old".into());
        let second = insert(&mut e, Some(&first), true);
        assert_ne!(first, second);
        assert_eq!(e.rows[1].text, "こんにちは");
        assert!(e.rows[1].take_id.is_none());
    }

    #[test]
    fn speaking_speed_maps_to_inverse_duration_and_survives_save() {
        for (speed, duration) in [(0.5, 2.0), (1.0, 1.0), (1.25, 0.8), (2.0, 0.5)] {
            let mut settings = GenerationSettings::default();
            settings.set_speed(speed);
            let input = speech("こんにちは".into(), &settings).unwrap();
            assert!((input.duration_scale - duration).abs() < 1e-12);
            let yaml = serde_yaml::to_string(&settings).unwrap();
            let restored: GenerationSettings = serde_yaml::from_str(&yaml).unwrap();
            assert!((restored.speed() - speed).abs() < 1e-12);
            assert!(yaml.contains("durationScale:"));
        }
        for speed in [0.0, 0.49, 2.01, f64::NAN, f64::INFINITY] {
            let mut settings = GenerationSettings::default();
            settings.set_speed(speed);
            assert!(speech("こんにちは".into(), &settings).is_err());
        }
    }
    #[test]
    fn reorder_and_delete_keep_neighbor_and_final_row() {
        let mut e = EditorState::default();
        let a = insert(&mut e, None, false);
        let b = insert(&mut e, None, false);
        let c = insert(&mut e, None, false);
        assert_eq!(move_row(&mut e.rows, &a, Some(&c)), Some(2));
        assert_eq!(
            e.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec![b.as_str(), c.as_str(), a.as_str()]
        );
        assert_eq!(remove(&mut e, &c), Some(a.clone()));
        assert_eq!(remove(&mut e, &a), Some(b.clone()));
        assert_eq!(remove(&mut e, &b), None);
    }

    #[test]
    fn move_row_preserves_content_and_ignores_unchanged_or_invalid_positions() {
        let mut e = EditorState::default();
        assert_eq!(move_row(&mut e.rows, "missing", None), None);
        let a = insert(&mut e, None, false);
        assert_eq!(move_row(&mut e.rows, &a, None), None);
        assert_eq!(move_row(&mut e.rows, &a, Some(&a)), None);
        let b = insert(&mut e, None, false);
        e.rows[1].text = "移動する台詞".into();
        e.rows[1].character_id = "character".into();
        e.rows[1].take_id = Some("generated-audio".into());
        e.rows[1].prompt = Some("明るく".into());
        let original = serde_yaml::to_string(&e).unwrap();
        let moved_row = serde_yaml::to_string(&e.rows[1]).unwrap();

        assert_eq!(move_row(&mut e.rows, &b, Some(&a)), None);
        assert_eq!(move_row(&mut e.rows, &b, Some("missing")), None);
        assert_eq!(serde_yaml::to_string(&e).unwrap(), original);
        assert_eq!(move_row(&mut e.rows, &b, None), Some(0));
        assert_eq!(serde_yaml::to_string(&e.rows[0]).unwrap(), moved_row);
        assert_eq!(e.rows[1].id, a);
        assert_eq!(move_row(&mut e.rows, &b, None), None);
        assert_eq!(move_row(&mut e.rows, &a, Some(&a)), None);
        assert_eq!(move_row(&mut e.rows, "missing", Some(&b)), None);

        assert_eq!(move_row(&mut e.rows, &b, Some(&a)), Some(1));
        assert_eq!(serde_yaml::to_string(&e).unwrap(), original);
    }
    #[test]
    fn validates_numbers_and_skips_blank_rows() {
        let mut s = GenerationSettings::default();
        assert_eq!(speech("文".into(), &s).unwrap().seed, None);
        for seed in ["-1", "1.5", "9007199254740992"] {
            s.seed = seed.into();
            assert!(speech("文".into(), &s).is_err());
        }
        s.seed = "9007199254740991".into();
        s.num_steps = "200".into();
        assert!(speech("文".into(), &s).is_ok());
        s.num_steps = "201".into();
        assert!(speech("文".into(), &s).is_err());
        assert!(
            requests(
                &[new_row("missing".into(), s)],
                &[],
                &GenerationSettings::default()
            )
            .unwrap()
            .is_empty()
        );
    }
}

/// Copy a generated take without carrying over its generated audio or seed.
pub fn row_from_take(take: &crate::domain::Take, characters: &[Character]) -> ScriptRow {
    ScriptRow {
        id: new_id(),
        text: take.text.clone(),
        prompt: Some(take.additional_prompt.clone()),
        character_id: take
            .character_id
            .as_ref()
            .filter(|id| characters.iter().any(|c| &c.id == *id))
            .cloned()
            .unwrap_or_default(),
        settings: None,
        take_id: None,
    }
}
/// Always targets the selected row, regardless of which settings panel is visible.
pub fn randomize_row(
    editor: &mut EditorState,
    id: &str,
    previous: Option<u64>,
    mut candidate: u32,
) -> Result<()> {
    let row = editor
        .rows
        .iter_mut()
        .find(|row| row.id == id)
        .ok_or_else(|| anyhow::anyhow!("行が見つかりません"))?;
    let settings = row
        .settings
        .get_or_insert_with(|| editor.preferences.clone());
    while Some(candidate as u64) == previous || settings.seed == candidate.to_string() {
        candidate = candidate.wrapping_add(1);
    }
    settings.seed = candidate.to_string();
    Ok(())
}
#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn random_seed_targets_row_and_preserves_defaults() {
        let mut editor = EditorState::default();
        editor.preferences.seed = "42".into();
        let id = insert(&mut editor, None, false);
        randomize_row(&mut editor, &id, Some(43), 42).unwrap();
        assert_eq!(editor.preferences.seed, "42");
        assert_eq!(editor.rows[0].settings.as_ref().unwrap().seed, "44");
    }
    #[test]
    fn copied_take_retains_speech_direction_without_old_audio() {
        let take = crate::domain::Take {
            id: new_id(),
            filename: None,
            row_id: new_id(),
            character_id: Some(new_id()),
            character_name: "削除済み".into(),
            description: "声".into(),
            additional_prompt: "小声で".into(),
            voice: None,
            text: "こんにちは".into(),
            model: "anime".into(),
            seed: 42,
            num_steps: 40,
            elapsed_ms: 1,
            audio_seconds: 1.0,
            created_at: 0,
        };
        let row = row_from_take(&take, &[]);
        assert_eq!(row.text, take.text);
        assert_eq!(row.prompt.as_deref(), Some("小声で"));
        assert!(row.character_id.is_empty());
        assert!(row.settings.is_none());
        assert!(row.take_id.is_none());
        assert_ne!(row.id, take.row_id);
    }
}
