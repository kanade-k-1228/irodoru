use super::*;
use crate::domain::{EditorState, Line};

#[test]
fn deleting_a_character_requires_no_dialogue_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let chara = library
        .create_chara(CharaInput {
            name: "キャラクター".into(),
            description: String::new(),
            voice_ids: Vec::new(),
            color: None,
            icon: None,
        })
        .unwrap();
    let mut editor = EditorState {
        lines: vec![Line {
            id: new_id(),
            chara_id: chara.id.clone(),
            text: String::new(),
            takes: Vec::new(),
            take_id: None,
            prompt: None,
            settings: None,
        }],
        ..Default::default()
    };
    library.save_editor(&editor).unwrap();
    let original = std::fs::read(dir.path().join("project.yaml")).unwrap();
    assert!(library.delete_chara(&chara.id).is_err());
    assert!(library.chara(&chara.id).is_ok());
    assert_eq!(
        std::fs::read(dir.path().join("project.yaml")).unwrap(),
        original
    );
    editor.lines[0].chara_id.clear();
    library.save_editor(&editor).unwrap();
    library.delete_chara(&chara.id).unwrap();
    assert!(library.chara(&chara.id).is_err());
}

#[test]
fn reference_playback_reads_duration_and_audio_from_character_owned_copies() {
    let directory = tempfile::tempdir().unwrap();
    let library = Library::open(directory.path()).unwrap();
    let mut wav = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut wav,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..48_001 * 2 {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
    let wav = wav.into_inner();
    let voice = library.save_voice("参照音声.wav", &wav).unwrap();
    let another_voice = library.save_voice("参照音声2.wav", &wav).unwrap();
    library.save_voice("未登録の音声.wav", &wav).unwrap();
    let chara = library
        .create_chara(CharaInput {
            name: "参照音声を持つキャラクター".into(),
            description: String::new(),
            voice_ids: vec![voice.clone(), another_voice.clone()],
            color: None,
            icon: None,
        })
        .unwrap();
    let audio = library.audio(&voice).unwrap();
    let another_audio = library.audio(&another_voice).unwrap();
    // Generation uses every recording belonging to this character, in upload order.
    assert_eq!(
        library.chara_reference_audio(&chara).unwrap(),
        vec![audio.clone(), another_audio.clone()]
    );
    let seconds = 48_001. / 48_000.;
    assert!((library.audio_seconds(&voice).unwrap() - seconds).abs() < 1e-9);
    // Character bundles have their own recordings without a shared library copy.
    std::fs::remove_file(library.voice_path(&voice).unwrap()).unwrap();
    assert!((library.audio_seconds(&voice).unwrap() - seconds).abs() < 1e-9);
    assert_eq!(library.audio(&voice).unwrap(), audio);
    assert_eq!(
        library.chara_reference_audio(&chara).unwrap(),
        vec![audio, another_audio]
    );
}
