use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

const SIMPLE_REPLACE: &[(&str, &str)] = &[
    ("\t", ""),
    ("[n]", ""),
    (r"\[n\]", ""),
    ("\u{3000}", ""),
    ("？", "?"),
    ("！", "!"),
    ("♥", "♡"),
    ("●", "○"),
    ("◯", "○"),
    ("〇", "○"),
];

static REGEX_REPLACE: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"[;▼♀♂《》≪≫①②③④⑤⑥]").unwrap(), ""),
        (
            Regex::new(r"[\x{02d7}\x{2010}-\x{2015}\x{2043}\x{2212}\x{23af}\x{23e4}\x{2500}\x{2501}\x{2e3a}\x{2e3b}]")
                .unwrap(),
            "",
        ),
        (Regex::new(r"[\x{ff5e}\x{301C}]").unwrap(), "ー"),
        (Regex::new(r"…{3,}").unwrap(), "……"),
    ]
});

fn strip_outer_brackets(text: &str) -> String {
    const PAIRS: &[(char, char)] = &[
        ('「', '」'),
        ('『', '』'),
        ('（', '）'),
        ('【', '】'),
        ('(', ')'),
    ];
    let chars: Vec<char> = text.chars().collect();
    let mut s = chars.as_slice();
    while let [open, .., close] = *s
        && PAIRS.contains(&(open, close))
        && encloses_all(s, open, close)
    {
        s = &s[1..s.len() - 1];
    }
    s.iter().collect()
}

fn encloses_all(s: &[char], open: char, close: char) -> bool {
    let mut depth = 0i64;
    for (i, &c) in s.iter().enumerate() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
        }
        if depth == 0 {
            return i == s.len() - 1;
        }
    }
    false
}

pub fn normalize_text(text: &str) -> String {
    let mut s = text.to_string();
    for (old, new) in SIMPLE_REPLACE {
        s = s.replace(old, new);
    }
    for (re, rep) in REGEX_REPLACE.iter() {
        s = re.replace_all(&s, *rep).into_owned();
    }
    let s = strip_outer_brackets(&s);
    let s: String = s.nfkc().collect();
    s.replace("...", "…").replace("..", "…")
}

pub struct TextTokenizer {
    inner: tokenizers::Tokenizer,
    forced: HashMap<u32, Vec<u32>>,
    bos: i64,
}

impl TextTokenizer {
    pub fn load(dir: &Path, file: &str, forced_splits: Option<&str>, bos: i64) -> Result<Self> {
        let inner = tokenizers::Tokenizer::from_file(dir.join(file))
            .map_err(|e| anyhow!("loading {file}: {e}"))?;
        let forced = match forced_splits {
            Some(f) => serde_json::from_str(
                &std::fs::read_to_string(dir.join(f)).with_context(|| f.to_string())?,
            )?,
            None => HashMap::new(),
        };
        Ok(Self { inner, forced, bos })
    }

    pub fn encode(&self, text: &str, max_len: usize) -> Result<Vec<i64>> {
        let enc = self
            .inner
            .encode(text, false)
            .map_err(|e| anyhow!("tokenize: {e}"))?;
        let mut ids = vec![self.bos];
        for &id in enc.get_ids() {
            match self.forced.get(&id) {
                Some(split) => ids.extend(split.iter().map(|&x| x as i64)),
                None => ids.push(id as i64),
            }
        }
        ids.truncate(max_len.max(1));
        Ok(ids)
    }
}
