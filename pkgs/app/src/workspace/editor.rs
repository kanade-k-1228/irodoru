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
