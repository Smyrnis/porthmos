use std::{path::PathBuf, sync::Arc};

use crate::{
    Severity,
    engine::{
        Command, Event,
        testing::{TestEngine, test_engine, test_engine_with_secrets},
    },
    import::{
        ImportPlan, ImportSource, Verdict,
        testing::{ftp_like, sftp_like},
    },
    profiles::store,
    secrets::TestBackend,
};

fn filezilla_server(name: &str, protocol: u8, extra: &str) -> String {
    format!(
        "<Server><Host>{name}.example</Host><Port>22</Port><Protocol>{protocol}</Protocol><User>alice</User><Logontype>1</Logontype><Name>{name}</Name>{extra}</Server>"
    )
}

fn filezilla(servers: &[String]) -> String {
    format!("<FileZilla3><Servers>{}</Servers></FileZilla3>", servers.concat())
}

fn engine() -> TestEngine {
    let mut t = test_engine();
    t.engine.protocols = vec![sftp_like(), ftp_like()];
    t
}

fn write_source(t: &TestEngine, text: &str) -> PathBuf {
    let path = t.dir.path().join("sitemanager.xml");
    std::fs::write(&path, text).unwrap();
    path
}

fn plan_of(events: Vec<Event>) -> Option<Arc<ImportPlan>> {
    events.into_iter().find_map(|event| match event {
        Event::ImportPlanReady(plan) => Some(plan),
        _ => None,
    })
}

async fn read(t: &mut TestEngine, import_id: u64, path: PathBuf) -> Option<Arc<ImportPlan>> {
    t.engine.handle_command(Command::ReadImport { import_id, source: ImportSource::FileZilla, path });
    t.run_internal().await;
    plan_of(t.drain())
}

fn saved_names(t: &TestEngine) -> Vec<String> {
    store::load(&t.engine.paths).unwrap().into_iter().map(|profile| profile.name).collect()
}

fn done_of(events: &[Event]) -> Option<(u64, usize, usize)> {
    events.iter().find_map(|event| match event {
        Event::ImportDone { import_id, added, skipped } => Some((*import_id, *added, *skipped)),
        _ => None,
    })
}

fn three_sites() -> String {
    filezilla(&[filezilla_server("alpha", 1, ""), filezilla_server("beta", 1, ""), filezilla_server("gamma", 1, "")])
}

#[tokio::test]
async fn read_import_emits_a_plan_with_new_rows() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    let plan = read(&mut t, 5, path).await.expect("a plan");
    assert_eq!((plan.import_id, plan.source), (5, ImportSource::FileZilla));
    assert_eq!(plan.rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(), ["alpha", "beta", "gamma"]);
    assert_eq!(plan.new_count(), 3);
    assert!(saved_names(&t).is_empty());
}

#[tokio::test]
async fn read_import_of_a_missing_file_fails_the_read_with_its_id_and_no_plan() {
    let mut t = engine();
    let missing = t.dir.path().join("nope.xml");
    t.engine.handle_command(Command::ReadImport { import_id: 7, source: ImportSource::FileZilla, path: missing });
    t.run_internal().await;
    let events = t.drain();
    assert!(plan_of(events.clone()).is_none());
    let failure = events.iter().find_map(|event| match event {
        Event::ImportReadFailed { import_id, message } => Some((*import_id, message.to_string())),
        _ => None,
    });
    let (import_id, text) = failure.expect("a read failure");
    assert_eq!(import_id, 7);
    assert!(text.contains("no such file or folder"));
}

#[tokio::test]
async fn a_failed_read_does_not_also_raise_an_error_notice() {
    let mut t = engine();
    let missing = t.dir.path().join("nope.xml");
    t.engine.handle_command(Command::ReadImport { import_id: 1, source: ImportSource::FileZilla, path: missing });
    t.run_internal().await;
    let events = t.drain();
    assert!(!events.iter().any(|event| matches!(event, Event::Notice { severity: Severity::Error, .. })));
}

#[tokio::test]
async fn existing_connections_are_marked_exists() {
    let mut t = engine();
    std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
    std::fs::write(t.engine.paths.connections_file(), "[connections.beta]\nhost = \"mine\"\nusername = \"me\"\n")
        .unwrap();
    let path = write_source(&t, &three_sites());
    let plan = read(&mut t, 1, path).await.unwrap();
    let verdicts: Vec<Verdict> = plan.rows.iter().map(|row| row.verdict.clone()).collect();
    assert_eq!(verdicts, [Verdict::New, Verdict::Exists, Verdict::New]);
}

#[tokio::test]
async fn run_import_saves_only_ticked_new_rows() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0, 2] });

    let events = t.drain();
    assert_eq!(done_of(&events), Some((1, 2, 0)));
    assert!(events.iter().any(|event| matches!(event, Event::Profiles(_))));
    assert_eq!(saved_names(&t), ["alpha", "gamma"]);
    let saved = store::load(&t.engine.paths).unwrap();
    assert_eq!(
        (saved[0].host.as_str(), saved[0].username.as_str(), saved[0].protocol.as_str()),
        ("alpha.example", "alice", "sftp")
    );
    assert_eq!(saved[0].port, Some(22));
}

#[tokio::test]
async fn run_import_never_overwrites_a_name_that_appeared_after_the_review() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
    std::fs::write(t.engine.paths.connections_file(), "[connections.alpha]\nhost = \"mine\"\nusername = \"me\"\n")
        .unwrap();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0, 1] });

    let events = t.drain();
    assert_eq!(done_of(&events), Some((1, 1, 1)));
    let saved = store::load(&t.engine.paths).unwrap();
    let alpha = saved.iter().find(|profile| profile.name == "alpha").unwrap();
    assert_eq!(alpha.host, "mine");
    assert!(saved.iter().any(|profile| profile.name == "beta"));
    let warning = events.iter().find_map(|event| match event {
        Event::Notice { severity: Severity::Warning, message } => Some(message.to_string()),
        _ => None,
    });
    assert!(warning.expect("a warning").starts_with("Skipped 1"));
}

#[tokio::test]
async fn run_import_skips_rows_that_are_not_new_and_bad_indices() {
    let mut t = engine();
    let text =
        filezilla(&[filezilla_server("good", 1, ""), filezilla_server("storj", 8, ""), filezilla_server("dup", 1, "")]);
    std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
    std::fs::write(t.engine.paths.connections_file(), "[connections.dup]\nhost = \"mine\"\nusername = \"me\"\n")
        .unwrap();
    let path = write_source(&t, &text);
    read(&mut t, 1, path).await.unwrap();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0, 1, 2, 999] });

    assert_eq!(done_of(&t.drain()), Some((1, 1, 3)));
    assert_eq!(saved_names(&t), ["dup", "good"]);
}

#[tokio::test]
async fn a_row_ticked_twice_is_saved_once() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![1, 1] });
    assert_eq!(done_of(&t.drain()), Some((1, 1, 1)));
    assert_eq!(saved_names(&t), ["beta"]);
}

#[tokio::test]
async fn a_stale_import_id_changes_nothing() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    t.engine.handle_command(Command::RunImport { import_id: 2, rows: vec![0] });
    assert!(done_of(&t.drain()).is_none());
    assert!(saved_names(&t).is_empty());
}

#[tokio::test]
async fn running_twice_saves_nothing_the_second_time() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0] });
    t.drain();
    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![1] });
    assert!(done_of(&t.drain()).is_none());
    assert_eq!(saved_names(&t), ["alpha"]);
}

#[tokio::test]
async fn cancel_import_drops_the_plan() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    t.engine.handle_command(Command::CancelImport { import_id: 1 });
    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0] });
    assert!(done_of(&t.drain()).is_none());
    assert!(saved_names(&t).is_empty());
}

#[tokio::test]
async fn cancelling_another_id_keeps_the_plan() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    t.engine.handle_command(Command::CancelImport { import_id: 9 });
    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0] });
    assert_eq!(done_of(&t.drain()), Some((1, 1, 0)));
}

#[tokio::test]
async fn a_second_read_replaces_the_first_plan() {
    let mut t = engine();
    let first = write_source(&t, &three_sites());
    read(&mut t, 1, first).await.unwrap();
    let second = t.dir.path().join("second.xml");
    std::fs::write(&second, filezilla(&[filezilla_server("only", 1, "")])).unwrap();
    read(&mut t, 2, second).await.unwrap();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0] });
    assert!(done_of(&t.drain()).is_none());
    t.engine.handle_command(Command::RunImport { import_id: 2, rows: vec![0] });
    assert_eq!(done_of(&t.drain()), Some((2, 1, 0)));
    assert_eq!(saved_names(&t), ["only"]);
}

#[tokio::test]
async fn a_result_for_a_replaced_read_is_ignored() {
    let mut t = engine();
    let first = write_source(&t, &three_sites());
    let second = t.dir.path().join("second.xml");
    std::fs::write(&second, filezilla(&[filezilla_server("only", 1, "")])).unwrap();
    t.engine.handle_command(Command::ReadImport { import_id: 1, source: ImportSource::FileZilla, path: first });
    t.engine.handle_command(Command::ReadImport { import_id: 2, source: ImportSource::FileZilla, path: second });
    t.settle().await;
    let plan = plan_of(t.drain()).expect("a plan");
    assert_eq!(plan.import_id, 2);
}

#[tokio::test]
async fn saved_profiles_have_no_keyring_marker_and_no_secret_calls() {
    let backend = Arc::new(TestBackend::new());
    let mut t = test_engine_with_secrets(backend.clone());
    t.engine.protocols = vec![sftp_like(), ftp_like()];
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    backend.calls();
    let calls_before = backend.calls().len();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0, 1, 2] });
    t.settle().await;

    assert_eq!(backend.calls().len(), calls_before);
    assert!(store::load(&t.engine.paths).unwrap().iter().all(|profile| profile.in_keyring.is_empty()));
}

#[tokio::test]
async fn import_never_leaks_source_secrets() {
    let secrets = "<Pass encoding=\"base64\">SENTINEL-PASSWORD-1234</Pass><Account>SENTINEL-ACCOUNT</Account><Parameter Name=\"x\">SENTINEL-PARAM</Parameter>";
    let mut t = engine();
    let source_dir = tempfile::tempdir().unwrap();
    let path = source_dir.path().join("sitemanager.xml");
    std::fs::write(&path, filezilla(&[filezilla_server("alpha", 1, secrets)])).unwrap();
    let mut events = Vec::new();
    t.engine.handle_command(Command::ReadImport { import_id: 1, source: ImportSource::FileZilla, path });
    t.run_internal().await;
    events.extend(t.drain());
    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0] });
    events.extend(t.drain());

    for sentinel in ["SENTINEL-PASSWORD-1234", "SENTINEL-ACCOUNT", "SENTINEL-PARAM"] {
        t.assert_secret_nowhere(sentinel, &events);
    }
}

#[tokio::test]
async fn a_write_failure_counts_as_skipped_and_stops_nothing_else() {
    let mut t = engine();
    let path = write_source(&t, &three_sites());
    read(&mut t, 1, path).await.unwrap();
    std::fs::create_dir_all(t.engine.paths.connections_file().with_extension("toml.tmp")).unwrap();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![0, 1] });

    let events = t.drain();
    assert_eq!(done_of(&events), Some((1, 0, 2)));
    assert!(saved_names(&t).is_empty());
}

#[tokio::test]
async fn a_failing_row_does_not_stop_the_rows_after_it() {
    let mut t = engine();
    let text = filezilla(&[filezilla_server("alpha", 1, ""), filezilla_server("beta", 1, "")]);
    let path = write_source(&t, &text);
    read(&mut t, 1, path).await.unwrap();

    t.engine.handle_command(Command::RunImport { import_id: 1, rows: vec![7, 0, 1] });

    assert_eq!(done_of(&t.drain()), Some((1, 2, 1)));
    assert_eq!(saved_names(&t), ["alpha", "beta"]);
}
