//! Editing rules shared by the native views and tests.
use crate::{
    domain::{Chara, new_id},
    domain::{EditorState, GenerationSettings, Line, SynthesisInput, TakeRequest},
};
use anyhow::{Result, bail};

pub fn new_line(chara_id: String, settings: GenerationSettings) -> Line {
    Line {
        takes: Vec::new(),
        id: new_id(),
        text: String::new(),
        chara_id,
        prompt: None,
        settings: Some(settings),
        take_id: None,
    }
}
pub fn insert(editor: &mut EditorState, selected: Option<&str>, duplicate: bool) -> String {
    let index = selected.and_then(|id| editor.lines.iter().position(|r| r.id == id));
    let source = index.map(|i| &editor.lines[i]);
    let mut line = new_line(
        source
            .map(|r| r.chara_id.clone())
            .unwrap_or_else(|| editor.active_chara.clone()),
        source.and_then(|r| r.settings.clone()).unwrap_or_default(),
    );
    if duplicate && let Some(source) = source {
        line.text = source.text.clone();
        line.prompt = source.prompt.clone();
    }
    let id = line.id.clone();
    editor
        .lines
        .insert(index.map_or(editor.lines.len(), |i| i + 1), line);
    id
}
/// Move a line after another line, or to the beginning when `after` is absent.
pub fn move_line(lines: &mut Vec<Line>, id: &str, after: Option<&str>) -> Option<usize> {
    let index = lines.iter().position(|line| line.id == id)?;
    let slot = match after {
        Some(after) => lines.iter().position(|line| line.id == after)? + 1,
        None => 0,
    };
    if slot == index || slot == index + 1 {
        return None;
    }
    let target = slot - usize::from(index < slot);
    let line = lines.remove(index);
    lines.insert(target, line);
    Some(target)
}
pub fn remove(editor: &mut EditorState, id: &str) -> Option<String> {
    if editor.lines.len() <= 1 {
        return None;
    }
    let index = editor.lines.iter().position(|r| r.id == id)?;
    editor.lines.remove(index);
    Some(editor.lines[index.min(editor.lines.len() - 1)].id.clone())
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
pub fn requests(lines: &[Line], charas: &[Chara]) -> Result<Vec<TakeRequest>> {
    let defaults = GenerationSettings::default();
    lines
        .iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| {
            if !charas.iter().any(|c| c.id == r.chara_id) {
                bail!("各セリフのプリセットを選択してください");
            }
            Ok(TakeRequest {
                line_id: r.id.clone(),
                chara_id: r.chara_id.clone(),
                additional_prompt: r
                    .prompt
                    .as_deref()
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned),
                request: speech(r.text.clone(), r.settings.as_ref().unwrap_or(&defaults))?,
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
        e.lines[0].text = "こんにちは".into();
        e.lines[0].take_id = Some("old".into());
        let second = insert(&mut e, Some(&first), true);
        assert_ne!(first, second);
        assert_eq!(e.lines[1].text, "こんにちは");
        assert!(e.lines[1].take_id.is_none());
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
            assert!(yaml.contains("duration_scale:"));
        }
        for speed in [0.0, 0.49, 2.01, f64::NAN, f64::INFINITY] {
            let mut settings = GenerationSettings::default();
            settings.set_speed(speed);
            assert!(speech("こんにちは".into(), &settings).is_err());
        }
    }
    #[test]
    fn reorder_and_delete_keep_neighbor_and_final_line() {
        let mut e = EditorState::default();
        let a = insert(&mut e, None, false);
        let b = insert(&mut e, None, false);
        let c = insert(&mut e, None, false);
        assert_eq!(move_line(&mut e.lines, &a, Some(&c)), Some(2));
        assert_eq!(
            e.lines.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec![b.as_str(), c.as_str(), a.as_str()]
        );
        assert_eq!(remove(&mut e, &c), Some(a.clone()));
        assert_eq!(remove(&mut e, &a), Some(b.clone()));
        assert_eq!(remove(&mut e, &b), None);
    }

    #[test]
    fn move_line_preserves_content_and_ignores_unchanged_or_invalid_positions() {
        let mut e = EditorState::default();
        assert_eq!(move_line(&mut e.lines, "missing", None), None);
        let a = insert(&mut e, None, false);
        assert_eq!(move_line(&mut e.lines, &a, None), None);
        assert_eq!(move_line(&mut e.lines, &a, Some(&a)), None);
        let b = insert(&mut e, None, false);
        e.lines[1].text = "移動する台詞".into();
        e.lines[1].chara_id = "chara".into();
        e.lines[1].take_id = Some("generated-audio".into());
        e.lines[1].prompt = Some("明るく".into());
        let original = serde_yaml::to_string(&e).unwrap();
        let moved_line = serde_yaml::to_string(&e.lines[1]).unwrap();

        assert_eq!(move_line(&mut e.lines, &b, Some(&a)), None);
        assert_eq!(move_line(&mut e.lines, &b, Some("missing")), None);
        assert_eq!(serde_yaml::to_string(&e).unwrap(), original);
        assert_eq!(move_line(&mut e.lines, &b, None), Some(0));
        assert_eq!(serde_yaml::to_string(&e.lines[0]).unwrap(), moved_line);
        assert_eq!(e.lines[1].id, a);
        assert_eq!(move_line(&mut e.lines, &b, None), None);
        assert_eq!(move_line(&mut e.lines, &a, Some(&a)), None);
        assert_eq!(move_line(&mut e.lines, "missing", Some(&b)), None);

        assert_eq!(move_line(&mut e.lines, &b, Some(&a)), Some(1));
        assert_eq!(serde_yaml::to_string(&e).unwrap(), original);
    }
    #[test]
    fn validates_numbers_and_skips_blank_lines() {
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
            requests(&[new_line("missing".into(), s)], &[],)
                .unwrap()
                .is_empty()
        );
    }
}

/// Copy a generated take without carrying over its generated audio or seed.
pub fn line_from_take(take: &crate::domain::Take, charas: &[Chara]) -> Line {
    Line {
        takes: Vec::new(),
        id: new_id(),
        text: take.conditions.text.clone(),
        prompt: Some(take.additional_prompt.clone()),
        chara_id: take
            .chara_id
            .as_ref()
            .filter(|id| charas.iter().any(|c| &c.id == *id))
            .cloned()
            .unwrap_or_default(),
        settings: None,
        take_id: None,
    }
}
/// Always targets the selected line, regardless of which settings panel is visible.
pub fn randomize_line(
    editor: &mut EditorState,
    id: &str,
    previous: Option<u64>,
    mut candidate: u32,
) -> Result<()> {
    let line = editor
        .lines
        .iter_mut()
        .find(|line| line.id == id)
        .ok_or_else(|| anyhow::anyhow!("行が見つかりません"))?;
    let settings = line
        .settings
        .get_or_insert_with(GenerationSettings::default);
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
    fn random_seed_targets_line_and_preserves_defaults() {
        let mut editor = EditorState::default();
        let id = insert(&mut editor, None, false);
        editor.lines[0].settings.as_mut().unwrap().seed = "42".into();
        randomize_line(&mut editor, &id, Some(43), 42).unwrap();
        let next = insert(&mut editor, None, false);
        assert!(
            editor
                .lines
                .iter()
                .find(|line| line.id == next)
                .unwrap()
                .settings
                .as_ref()
                .unwrap()
                .seed
                .is_empty()
        );
        assert_eq!(editor.lines[0].settings.as_ref().unwrap().seed, "44");
    }
    #[test]
    fn copied_take_retains_speech_direction_without_old_audio() {
        let take = crate::domain::Take {
            id: new_id(),
            conditions: irodoru_engine::GenerationConditions {
                text: "こんにちは".into(),
                model: "anime".into(),
                seed: 42,
                num_steps: 40,
                ..crate::domain::Take::test_conditions()
            },
            line_id: new_id(),
            chara_id: Some(new_id()),
            chara_name: "削除済み".into(),
            additional_prompt: "小声で".into(),
            voice_ids: Vec::new(),
            elapsed_ms: 1,
            audio_seconds: 1.0,
            created_at: 0,
        };
        let line = line_from_take(&take, &[]);
        assert_eq!(line.text, take.conditions.text);
        assert_eq!(line.prompt.as_deref(), Some("小声で"));
        assert!(line.chara_id.is_empty());
        assert!(line.settings.is_none());
        assert!(line.take_id.is_none());
        assert_ne!(line.id, take.line_id);
    }
}
