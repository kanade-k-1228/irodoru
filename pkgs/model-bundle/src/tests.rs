use super::*;

fn distribution() -> Distribution {
    Distribution {
        schema: 2,
        model: "anime".into(),
        revision: Some("1".repeat(64)),
        repository: Some("example/models".into()),
        archive: Some(Archive {
            format: "zip".into(),
            asset: "models.zip".into(),
            size: 1024,
            sha256: "2".repeat(64),
        }),
        files: required_paths()
            .map(|path| Entry {
                path,
                size: 1,
                sha256: "3".repeat(64),
            })
            .collect(),
    }
}

#[test]
fn published_offline_and_unpublished_contracts() {
    let mut manifest = distribution();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let parsed: Distribution = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.validate().unwrap(), manifest.files.len() as u64);
    manifest.repository = None;
    assert!(
        manifest.validate().is_ok(),
        "an existing cache needs no registry"
    );
    let placeholder: Distribution = serde_json::from_str(r#"{"schema":2,"model":"anime","revision":null,"repository":null,"archive":null,"files":[]}"#).unwrap();
    assert!(placeholder.is_unpublished());
    assert!(placeholder.validate().is_err());
}

#[test]
fn rejects_unsafe_duplicate_missing_and_corrupt_entries() {
    for path in [
        "../escape",
        "anime/../escape",
        "anime/.hidden",
        "anime/a/b",
        "anime/C:x",
        "anime/file.",
        "anime/\\escape",
    ] {
        let mut manifest = distribution();
        manifest.files[0].path = path.into();
        assert!(manifest.validate().is_err(), "{path}");
    }
    for change in [
        "duplicate",
        "missing",
        "empty",
        "overflow",
        "hash",
        "schema",
        "model",
        "revision",
        "repository",
        "archive",
        "total",
    ] {
        let mut m = distribution();
        match change {
            "duplicate" => m.files.push(m.files[0].clone()),
            "missing" => {
                m.files.pop();
            }
            "empty" => m.files[0].size = 0,
            "overflow" => m.files[0].size = u64::MAX,
            "hash" => m.files[0].sha256 = "A".repeat(64),
            "schema" => m.schema = 1,
            "model" => m.model = "other".into(),
            "revision" => m.revision = Some("../escape".into()),
            "repository" => m.repository = Some("Owner/private/more".into()),
            "archive" => m.archive.as_mut().unwrap().asset = "../escape.zip".into(),
            "total" => m.archive.as_mut().unwrap().size = 1,
            _ => unreachable!(),
        }
        assert!(m.validate().is_err(), "{change}");
    }
}
