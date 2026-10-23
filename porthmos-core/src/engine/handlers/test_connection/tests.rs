use std::{sync::Arc, time::Duration};

use porthmos_vfs::{
    Answer, ErrorKind, Question,
    testing::{FakeFs, FakeProtocol},
};

use crate::{
    NotTrusted, StartFolder, TestOutcome, TestReport, TestStage,
    engine::{
        Command, Event,
        testing::{TestEngine, test_engine},
    },
    profiles::{ProfileDraft, SecretEdit},
};

fn draft() -> ProfileDraft {
    ProfileDraft {
        protocol: "fake".to_string(),
        name: "box".to_string(),
        host: "h".to_string(),
        username: "u".to_string(),
        password: SecretEdit::Replace("pw".to_string()),
        ..ProfileDraft::default()
    }
}

fn engine_with(protocol: FakeProtocol) -> TestEngine {
    let mut t = test_engine();
    t.engine.protocols = vec![Arc::new(protocol)];
    t
}

fn start(t: &mut TestEngine, test_id: u64, draft: ProfileDraft) {
    t.engine.handle_command(Command::TestConnection { test_id, original: None, draft: Box::new(draft) });
}

async fn next_finished(t: &mut TestEngine) -> (u64, TestOutcome) {
    loop {
        if let Event::TestFinished { test_id, outcome } = t.next_event().await {
            return (test_id, outcome);
        }
    }
}

async fn outcome_of(t: &mut TestEngine, draft: ProfileDraft) -> TestOutcome {
    start(t, 1, draft);
    next_finished(t).await.1
}

fn connected(outcome: TestOutcome) -> TestReport {
    match outcome {
        TestOutcome::Connected(report) => report,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_successful_test_emits_connected_with_server_info_and_an_opened_folder() {
    let fs = FakeFs::new();
    fs.dir("/srv").with_server_info("OpenSSH_9.6");
    let mut t = engine_with(FakeProtocol::new(fs));
    let mut draft = draft();
    draft.remote_path = "/srv".to_string();

    let report = connected(outcome_of(&mut t, draft).await);

    assert_eq!(report.server.as_deref(), Some("OpenSSH_9.6"));
    assert_eq!(report.start_folder, StartFolder::Opens("/srv".to_string()));
}

#[tokio::test]
async fn the_outcome_carries_the_test_id() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()));

    start(&mut t, 42, draft());

    assert_eq!(next_finished(&mut t).await.0, 42);
}

#[tokio::test]
async fn a_login_failure_is_failed_at_login() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).failing_with(ErrorKind::AuthRejected, "no"));

    match outcome_of(&mut t, draft()).await {
        TestOutcome::Failed { stage, message } => {
            assert_eq!(stage, TestStage::Login);
            assert!(message.contains("Authentication failed for box"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_refused_connection_is_failed_at_connect() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).failing_connect("connection refused"));

    match outcome_of(&mut t, draft()).await {
        TestOutcome::Failed { stage, message } => {
            assert_eq!(stage, TestStage::Connect);
            assert!(message.contains("connection refused"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_declined_unknown_host_key_is_not_trusted_with_its_fingerprint_and_key_type() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).asking_to_trust_host_key("ssh-ed25519", "SHA256:abc123"));

    let outcome = outcome_of(&mut t, draft()).await;

    assert_eq!(
        outcome,
        TestOutcome::NotTrusted(NotTrusted::HostKey {
            key_type: "ssh-ed25519".to_string(),
            fingerprint: "SHA256:abc123".to_string()
        })
    );
    assert!(t.drain().iter().all(|event| !matches!(event, Event::Question { .. } | Event::TestQuestion { .. })));
}

#[tokio::test]
async fn a_declined_certificate_is_not_trusted_with_fingerprint_and_subject() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).asking_to_trust_certificate("AA:BB", "CN=files.example"));

    let outcome = outcome_of(&mut t, draft()).await;

    assert_eq!(
        outcome,
        TestOutcome::NotTrusted(NotTrusted::Certificate {
            fingerprint: "AA:BB".to_string(),
            subject: "CN=files.example".to_string()
        })
    );
}

#[tokio::test]
async fn a_changed_host_key_error_is_failed_at_connect_with_the_message() {
    let mut t = engine_with(
        FakeProtocol::new(FakeFs::new())
            .failing_connect("REMOTE HOST IDENTIFICATION HAS CHANGED for h:22 \u{2014} refusing to connect"),
    );

    match outcome_of(&mut t, draft()).await {
        TestOutcome::Failed { stage: TestStage::Connect, message } => {
            assert!(message.contains("REMOTE HOST IDENTIFICATION HAS CHANGED"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_password_question_is_forwarded_as_a_test_question_and_the_answer_reaches_the_protocol() {
    let protocol = FakeProtocol::new(FakeFs::new()).requiring_password("secret");
    let mut t = engine_with(protocol);
    let mut draft = draft();
    draft.password = SecretEdit::Clear;
    start(&mut t, 5, draft);

    let (test_id, request_id, question) = loop {
        if let Event::TestQuestion { test_id, request_id, question } = t.next_event().await {
            break (test_id, request_id, question);
        }
    };
    assert_eq!(test_id, 5);
    assert_eq!(question, Question::Password { username: "u".to_string(), name: "box".to_string() });
    t.engine.handle_command(Command::Answer {
        request_id,
        answer: Some(Answer::Password("secret".to_string())),
        save: true,
    });

    assert!(matches!(next_finished(&mut t).await.1, TestOutcome::Connected(_)));
}

#[tokio::test]
async fn cancelling_the_password_question_ends_the_test_as_cancelled() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).requiring_password("secret"));
    let mut draft = draft();
    draft.password = SecretEdit::Clear;
    start(&mut t, 5, draft);
    let request_id = loop {
        if let Event::TestQuestion { request_id, .. } = t.next_event().await {
            break request_id;
        }
    };

    t.engine.handle_command(Command::Answer { request_id, answer: None, save: false });

    assert_eq!(next_finished(&mut t).await, (5, TestOutcome::Cancelled));
}

#[tokio::test]
async fn the_filesystem_is_dropped_after_the_test() {
    let fs = FakeFs::new();
    let baseline_holder = fs.clone();
    let mut t = engine_with(FakeProtocol::new(fs));
    let baseline = baseline_holder.handles();

    connected(outcome_of(&mut t, draft()).await);
    tokio::task::yield_now().await;

    assert_eq!(baseline_holder.handles(), baseline);
}

#[tokio::test]
async fn the_test_does_not_create_a_session_or_emit_connecting_or_connected() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()));

    connected(outcome_of(&mut t, draft()).await);

    assert!(t.engine.sessions.is_empty());
    assert!(t.engine.connecting.is_empty());
    assert!(t.drain().iter().all(|event| !matches!(
        event,
        Event::Connecting { .. } | Event::Connected { .. } | Event::ConnectFailed { .. }
    )));
}

#[tokio::test(start_paused = true)]
async fn a_second_test_cancels_the_first() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).with_connect_delay(Duration::from_secs(5)));
    start(&mut t, 1, draft());
    tokio::task::yield_now().await;

    start(&mut t, 2, draft());

    let mut seen = Vec::new();
    while seen.len() < 2 {
        if let Some(Event::TestFinished { test_id, outcome }) = t.events.recv().await {
            seen.push((test_id, outcome));
        }
    }
    assert_eq!(seen[0], (1, TestOutcome::Cancelled));
    assert_eq!(seen[1].0, 2);
    assert!(matches!(seen[1].1, TestOutcome::Connected(_)));
}

#[tokio::test(start_paused = true)]
async fn cancel_test_ends_a_running_test_as_cancelled_and_drops_the_connection() {
    let fs = FakeFs::new();
    let holder = fs.clone();
    let mut t = engine_with(FakeProtocol::new(fs).with_connect_delay(Duration::from_secs(5)));
    let baseline = holder.handles();
    start(&mut t, 3, draft());
    tokio::task::yield_now().await;

    t.engine.handle_command(Command::CancelTest);

    let outcome = loop {
        if let Some(Event::TestFinished { outcome, .. }) = t.events.recv().await {
            break outcome;
        }
    };
    assert_eq!(outcome, TestOutcome::Cancelled);
    tokio::task::yield_now().await;
    assert_eq!(holder.handles(), baseline);
}

#[tokio::test(start_paused = true)]
async fn shutdown_cancels_a_running_test() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()).with_connect_delay(Duration::from_secs(5)));
    start(&mut t, 3, draft());
    tokio::task::yield_now().await;

    t.engine.tasks.cancel_all();

    let outcome = loop {
        if let Some(Event::TestFinished { outcome, .. }) = t.events.recv().await {
            break outcome;
        }
    };
    assert_eq!(outcome, TestOutcome::Cancelled);
}

mod hardening {
    use std::{collections::BTreeMap, path::PathBuf};

    use porthmos_vfs::{ConnectionForm, OptionField, OptionKind};

    use super::*;
    use crate::{
        engine::testing::test_engine_with_secrets,
        profiles::ConnectionEntry,
        secrets::TestBackend,
        test_connection::{TEST_LIMIT, TestStage},
    };

    const SAVED: &str = "in_keyring = [\"password\", \"token\"]\n";

    fn form_with_token() -> ConnectionForm {
        let mut form = ConnectionForm::standard(2222);
        form.options.push(OptionField { key: "token", label: "Token", required: false, kind: OptionKind::Secret });
        form
    }

    fn saved_engine(backend: &Arc<TestBackend>, protocol: FakeProtocol, lines: &str) -> TestEngine {
        let mut t = test_engine_with_secrets(backend.clone());
        std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
        std::fs::write(
            t.engine.paths.connections_file(),
            format!("[connections.srv]\nprotocol = \"fake\"\nhost = \"h\"\nusername = \"u\"\n{lines}"),
        )
        .unwrap();
        t.engine.protocols = vec![Arc::new(protocol)];
        t
    }

    fn editing(name: &str) -> ProfileDraft {
        ProfileDraft {
            protocol: "fake".to_string(),
            name: name.to_string(),
            host: "h".to_string(),
            username: "u".to_string(),
            password: SecretEdit::Keep,
            ..ProfileDraft::default()
        }
    }

    fn start_editing(t: &mut TestEngine, test_id: u64, draft: ProfileDraft) {
        t.engine.handle_command(Command::TestConnection {
            test_id,
            original: Some("srv".to_string()),
            draft: Box::new(draft),
        });
    }

    fn snapshot(dir: &std::path::Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut found = BTreeMap::new();
        let mut folders = vec![dir.to_path_buf()];
        while let Some(folder) = folders.pop() {
            for item in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = item.path();
                if path.is_dir() {
                    folders.push(path);
                } else {
                    found.insert(path.clone(), std::fs::read(&path).unwrap());
                }
            }
        }
        found
    }

    async fn first_question(t: &mut TestEngine) -> u64 {
        loop {
            if let Event::TestQuestion { request_id, .. } = t.events.recv().await.expect("an open event channel") {
                return request_id;
            }
        }
    }

    async fn finished_raw(t: &mut TestEngine) -> TestOutcome {
        loop {
            if let Event::TestFinished { outcome, .. } = t.events.recv().await.expect("an open event channel") {
                return outcome;
            }
        }
    }

    #[tokio::test]
    async fn a_draft_without_a_host_ends_as_failed_at_settings_with_the_save_message() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()));
        let mut draft = draft();
        draft.host = String::new();
        let expected = draft.validate(&ConnectionForm::standard(2222), None).unwrap_err();

        let outcome = outcome_of(&mut t, draft).await;

        assert_eq!(outcome, TestOutcome::Failed { stage: TestStage::Settings, message: expected });
    }

    #[tokio::test]
    async fn a_bad_port_gives_the_same_message_as_saving() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()));
        let mut draft = draft();
        draft.port = "99999".to_string();
        let expected = draft.validate(&ConnectionForm::standard(2222), None).unwrap_err();

        assert_eq!(
            outcome_of(&mut t, draft).await,
            TestOutcome::Failed { stage: TestStage::Settings, message: expected }
        );
    }

    #[tokio::test]
    async fn an_unknown_protocol_is_a_settings_failure_and_no_profile_rejected_event_is_emitted() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()));
        let mut draft = draft();
        draft.protocol = "nope".to_string();
        draft.host = String::new();

        let outcome = outcome_of(&mut t, draft).await;

        assert!(matches!(outcome, TestOutcome::Failed { stage: TestStage::Settings, .. }));
        assert!(t.drain().iter().all(|event| !matches!(event, Event::ProfileRejected { .. })));
    }

    #[tokio::test]
    async fn a_kept_password_is_looked_up_for_the_same_protocol() {
        let backend = Arc::new(TestBackend::new());
        backend.put("profile:srv", "hunter2");
        let protocol = FakeProtocol::new(FakeFs::new()).requiring_password("hunter2");
        let mut t = saved_engine(&backend, protocol, "in_keyring = [\"password\"]\n");

        start_editing(&mut t, 1, editing("srv"));

        assert!(matches!(finished_raw(&mut t).await, TestOutcome::Connected(_)));
        assert_eq!(backend.calls(), vec!["get profile:srv"]);
        assert!(t.drain().iter().all(|event| !matches!(event, Event::TestQuestion { .. })));
    }

    #[tokio::test]
    async fn kept_option_secrets_are_looked_up() {
        let backend = Arc::new(TestBackend::new());
        backend.put("profile:srv", "hunter2");
        backend.put(&super::super::super::profiles::profile_account("srv", "token"), "tok-123");
        let protocol = FakeProtocol::new(FakeFs::new()).with_form(form_with_token());
        let seen = protocol.seen_targets();
        let mut t = saved_engine(&backend, protocol, SAVED);
        let mut draft = editing("srv");
        draft.secret_options.insert("token".to_string(), SecretEdit::Keep);

        start_editing(&mut t, 1, draft);

        assert!(matches!(finished_raw(&mut t).await, TestOutcome::Connected(_)));
        let target = seen.lock().unwrap().clone().unwrap();
        assert_eq!(target.password.as_deref(), Some("hunter2"));
        assert_eq!(target.option("token"), Some("tok-123"));
    }

    #[tokio::test]
    async fn a_kept_secret_is_dropped_when_the_protocol_changed() {
        let backend = Arc::new(TestBackend::new());
        backend.put("profile:srv", "hunter2");
        let mut t = saved_engine(&backend, FakeProtocol::new(FakeFs::new()), "in_keyring = [\"password\"]\n");
        t.engine.protocols = vec![
            Arc::new(FakeProtocol::new(FakeFs::new())),
            Arc::new(FakeProtocol::new(FakeFs::new()).with_id("other").requiring_password("x")),
        ];
        let mut draft = editing("srv");
        draft.protocol = "other".to_string();
        start_editing(&mut t, 1, draft);

        let request_id = first_question(&mut t).await;

        t.engine.handle_command(Command::Answer { request_id, answer: None, save: false });
        assert_eq!(finished_raw(&mut t).await, TestOutcome::Cancelled);
        assert!(backend.calls().is_empty(), "{:?}", backend.calls());
    }

    #[tokio::test]
    async fn a_cleared_secret_is_not_sent_and_the_keyring_is_not_read() {
        let backend = Arc::new(TestBackend::new());
        backend.put("profile:srv", "hunter2");
        let protocol = FakeProtocol::new(FakeFs::new());
        let seen = protocol.seen_targets();
        let mut t = saved_engine(&backend, protocol, "in_keyring = [\"password\"]\n");
        let mut draft = editing("srv");
        draft.password = SecretEdit::Clear;

        start_editing(&mut t, 1, draft);
        finished_raw(&mut t).await;

        assert_eq!(seen.lock().unwrap().clone().unwrap().password, None);
        assert!(backend.calls().is_empty());
    }

    #[tokio::test]
    async fn a_replaced_secret_is_used_and_never_stored() {
        let backend = Arc::new(TestBackend::new());
        backend.put("profile:srv", "old");
        let protocol = FakeProtocol::new(FakeFs::new());
        let seen = protocol.seen_targets();
        let mut t = saved_engine(&backend, protocol, "in_keyring = [\"password\"]\n");
        let before = snapshot(t.dir.path());
        let mut draft = editing("srv");
        draft.password = SecretEdit::Replace("typed-secret".to_string());

        start_editing(&mut t, 1, draft);
        finished_raw(&mut t).await;

        assert_eq!(seen.lock().unwrap().clone().unwrap().password.as_deref(), Some("typed-secret"));
        assert_eq!(backend.stored("profile:srv").as_deref(), Some("old"));
        assert!(backend.calls().is_empty());
        assert_eq!(snapshot(t.dir.path()), before);
        let events = t.drain();
        t.assert_secret_nowhere("typed-secret", &events);
    }

    #[tokio::test]
    async fn a_typed_password_is_not_kept_after_the_attempt() {
        let backend = Arc::new(TestBackend::new());
        let mut t = saved_engine(&backend, FakeProtocol::new(FakeFs::new()).requiring_password("typed-pw"), "");
        let mut draft = draft();
        draft.password = SecretEdit::Clear;
        start(&mut t, 1, draft);
        let request_id = first_question(&mut t).await;

        t.engine.handle_command(Command::Answer {
            request_id,
            answer: Some(Answer::Password("typed-pw".to_string())),
            save: true,
        });

        assert!(matches!(finished_raw(&mut t).await, TestOutcome::Connected(_)));
        assert!(backend.calls().is_empty());
        assert_eq!(t.engine.secrets.lookup("profile:box", false).await.map(|secret| secret.to_string()), None);
        let events = t.drain();
        t.assert_secret_nowhere("typed-pw", &events);
    }

    #[tokio::test]
    async fn the_test_leaves_the_stores_and_known_hosts_unchanged_when_everything_goes_through() {
        let backend = Arc::new(TestBackend::new());
        let mut t = saved_engine(
            &backend,
            FakeProtocol::new(FakeFs::new())
                .asking_to_trust_host_key("ssh-ed25519", "SHA256:x")
                .requiring_password("pw"),
            "",
        );
        std::fs::create_dir_all(t.dir.path().join(".ssh")).unwrap();
        std::fs::write(t.dir.path().join(".ssh/known_hosts"), b"example ssh-ed25519 AAAA\n").unwrap();
        let before = snapshot(t.dir.path());
        let mut draft = draft();
        draft.password = SecretEdit::Clear;

        start(&mut t, 1, draft);
        let outcome = finished_raw(&mut t).await;

        assert!(matches!(outcome, TestOutcome::NotTrusted(_)));
        assert_eq!(snapshot(t.dir.path()), before);
        assert!(backend.calls().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_for_a_password_does_not_change_elapsed() {
        let protocol =
            FakeProtocol::new(FakeFs::new()).requiring_password("pw").with_login_delay(Duration::from_secs(3));
        let mut t = engine_with(protocol);
        let mut draft = draft();
        draft.password = SecretEdit::Clear;
        start(&mut t, 1, draft);
        let request_id = first_question(&mut t).await;

        tokio::time::advance(Duration::from_secs(20)).await;
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        t.engine.handle_command(Command::Answer {
            request_id,
            answer: Some(Answer::Password("pw".to_string())),
            save: false,
        });

        let report = connected(finished_raw(&mut t).await);
        assert!(
            report.elapsed >= Duration::from_secs(3) && report.elapsed < Duration::from_millis(3200),
            "{:?}",
            report.elapsed
        );
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_for_a_password_does_not_use_up_the_limit() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()).requiring_password("pw"));
        let mut draft = draft();
        draft.password = SecretEdit::Clear;
        start(&mut t, 1, draft);
        let request_id = first_question(&mut t).await;

        tokio::time::advance(Duration::from_secs(40)).await;
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        t.engine.handle_command(Command::Answer {
            request_id,
            answer: Some(Answer::Password("pw".to_string())),
            save: false,
        });

        assert!(matches!(finished_raw(&mut t).await, TestOutcome::Connected(_)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_connect_that_hangs_times_out_after_fifteen_seconds_of_active_time() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()).with_connect_delay(Duration::from_secs(600)));
        let before = tokio::time::Instant::now();
        start(&mut t, 1, draft());

        let outcome = finished_raw(&mut t).await;

        assert_eq!(outcome, TestOutcome::TimedOut(TestStage::Connect));
        let waited = before.elapsed();
        assert!(waited >= TEST_LIMIT && waited < TEST_LIMIT + Duration::from_millis(200), "{waited:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_login_that_hangs_reports_login_as_the_stage() {
        let protocol =
            FakeProtocol::new(FakeFs::new()).requiring_password("pw").with_login_delay(Duration::from_secs(600));
        let mut t = engine_with(protocol);
        let mut draft = draft();
        draft.password = SecretEdit::Clear;
        start(&mut t, 1, draft);
        let request_id = first_question(&mut t).await;
        t.engine.handle_command(Command::Answer {
            request_id,
            answer: Some(Answer::Password("pw".to_string())),
            save: false,
        });

        assert_eq!(finished_raw(&mut t).await, TestOutcome::TimedOut(TestStage::Login));
    }

    #[tokio::test(start_paused = true)]
    async fn a_hanging_stat_times_out_in_the_start_folder_stage() {
        let fs = FakeFs::new();
        fs.dir("/slow").hanging_stat("/slow");
        let mut t = engine_with(FakeProtocol::new(fs));
        let mut draft = draft();
        draft.remote_path = "/slow".to_string();
        start(&mut t, 1, draft);

        assert_eq!(finished_raw(&mut t).await, TestOutcome::TimedOut(TestStage::StartFolder));
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_but_successful_login_reports_its_real_time() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()).with_connect_delay(Duration::from_secs(4)));
        start(&mut t, 1, draft());

        let report = connected(finished_raw(&mut t).await);

        assert!(
            report.elapsed >= Duration::from_secs(4) && report.elapsed < Duration::from_millis(4200),
            "{:?}",
            report.elapsed
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_real_connect_to_the_same_name_works_during_and_after_a_test() {
        let backend = Arc::new(TestBackend::new());
        let protocol = FakeProtocol::new(FakeFs::new()).with_connect_delay(Duration::from_secs(1));
        let mut t = saved_engine(&backend, protocol, "");
        start_editing(&mut t, 1, editing_plain());
        tokio::task::yield_now().await;

        t.engine.handle_command(Command::Connect { profile: "srv".into() });
        assert!(t.engine.connecting.contains("srv"));
        t.run_internal().await;
        let mut seen = t.drain();
        while !seen.iter().any(|event| matches!(event, Event::TestFinished { .. })) {
            seen.push(t.events.recv().await.expect("an open event channel"));
        }

        assert!(seen.iter().any(|event| matches!(event, Event::Connected { name, .. } if name == "srv")));
        assert!(
            seen.iter().any(|event| matches!(event, Event::TestFinished { outcome: TestOutcome::Connected(_), .. }))
        );
        assert!(t.engine.connecting.is_empty());
        assert_eq!(t.engine.sessions.len(), 1);
    }

    fn editing_plain() -> ProfileDraft {
        let mut draft = editing("srv");
        draft.password = SecretEdit::Clear;
        draft
    }

    #[tokio::test]
    async fn the_tests_cancelled_question_is_forgotten() {
        let mut t = engine_with(FakeProtocol::new(FakeFs::new()).requiring_password("pw"));
        let mut draft = draft();
        draft.password = SecretEdit::Clear;
        start(&mut t, 1, draft);
        let request_id = first_question(&mut t).await;
        assert_eq!(t.engine.questions.pending(), 1);

        t.engine.handle_command(Command::CancelTest);

        assert_eq!(finished_raw(&mut t).await, TestOutcome::Cancelled);
        assert_eq!(t.engine.questions.pending(), 0);
        t.engine.handle_command(Command::Answer { request_id, answer: None, save: false });
        let _ = ConnectionEntry::target;
    }
}

#[tokio::test]
async fn a_connection_test_leaves_the_last_used_time_alone() {
    let mut t = engine_with(FakeProtocol::new(FakeFs::new()));
    t.drain();

    connected(outcome_of(&mut t, draft()).await);

    assert!(t.engine.last_used.store.times().is_empty());
    assert!(t.drain().iter().all(|event| !matches!(event, Event::LastUsed(_))));
}
