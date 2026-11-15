use porthmos_core::{Entry, Question};

use porthmos_core::delete_prompt::MAX_LISTED_NAMES;

use porthmos_core::delete_prompt::count::{ContentCount, Contents};

use super::{DeleteCount, Modal, ModalQueue, delete_lines, delete_summary};
use crate::app::PanelSide;
use crate::dialogs::Dialog;

fn entry(name: &str) -> Entry {
    Entry { name: name.to_string(), ..Entry::default() }
}

fn question(id: u64) -> Modal {
    Modal::Question(Dialog::from_question(
        id,
        Question::Password { username: "u".to_string(), name: "n".to_string(), secret: "Password" },
        None,
    ))
}

#[test]
fn the_queue_is_first_in_first_out() {
    let mut queue = ModalQueue::default();
    assert!(queue.is_empty());
    queue.push(question(1));
    queue.push(Modal::NewFolder { side: PanelSide::Local, text: String::new(), error: None });

    assert!(matches!(queue.front(), Some(Modal::Question(_))));
    assert!(matches!(queue.pop(), Some(Modal::Question(_))));
    assert!(matches!(queue.front(), Some(Modal::NewFolder { .. })));
}

#[test]
fn the_version_changes_when_the_front_modal_changes() {
    let mut queue = ModalQueue::default();
    let empty = queue.version();
    queue.push(question(1));
    let one = queue.version();
    queue.push(question(2));
    assert_eq!(queue.version(), one);
    queue.pop();
    assert_ne!(queue.version(), one);
    assert_ne!(one, empty);
}

#[test]
fn the_delete_summary_gives_a_count_and_at_most_five_names() {
    let few = [entry("a"), entry("b")];
    assert_eq!(delete_summary(&few, "prod"), ["Delete 2 items from prod?", "a", "b"]);

    let one = [entry("only")];
    assert_eq!(delete_summary(&one, "prod"), ["Delete \"only\" from prod?"]);

    let many: Vec<Entry> = (0..50_000).map(|n| entry(&format!("f{n}"))).collect();
    let lines = delete_summary(&many, "prod");
    assert_eq!(lines.len(), 1 + MAX_LISTED_NAMES + 1);
    assert_eq!(lines[0], "Delete 50000 items from prod?");
    assert_eq!(lines.last().unwrap(), "and 49995 more");
}

#[test]
fn a_delete_summary_names_folders_with_a_slash_and_cuts_odd_names() {
    let mut folder = entry("docs");
    folder.is_dir = true;
    let odd = entry("a\nb");

    assert_eq!(
        delete_summary(&[folder, odd], "this computer"),
        ["Delete 2 items from this computer?", "docs/", "a·b", "Folders are deleted with everything inside them."]
    );
}

#[test]
fn any_and_remove_where_find_queued_modals() {
    let mut queue = ModalQueue::default();
    queue.push(question(1));
    queue.push(Modal::EditWaiting { edit_id: 7, name: "a".to_string() });

    assert!(queue.any(|modal| matches!(modal, Modal::EditWaiting { edit_id: 7, .. })));
    assert!(!queue.any(|modal| matches!(modal, Modal::EditWaiting { edit_id: 8, .. })));
    assert!(queue.remove_where(|modal| matches!(modal, Modal::EditWaiting { edit_id: 7, .. })));
    assert!(!queue.remove_where(|modal| matches!(modal, Modal::EditWaiting { .. })));
    assert!(matches!(queue.front(), Some(Modal::Question(_))));
}

#[test]
fn removing_the_front_modal_changes_the_version() {
    let mut queue = ModalQueue::default();
    queue.push(Modal::EditWaiting { edit_id: 7, name: "a".to_string() });
    let before = queue.version();

    queue.remove_where(|_| true);

    assert_ne!(queue.version(), before);
    assert!(queue.is_empty());
}

#[test]
fn edit_dialog_titles_make_hostile_names_printable() {
    use porthmos_core::edit::EditQuestionKind;

    let hostile = "evil\u{1b}[31m\nname";
    for title in [
        super::edit_question_title(hostile, EditQuestionKind::Upload),
        super::edit_question_title(hostile, EditQuestionKind::Conflict),
    ] {
        assert!(!title.contains('\u{1b}') && !title.contains('\n'), "{title:?}");
    }
    assert_eq!(super::edit_question_title("a.txt", EditQuestionKind::Upload), "Upload your changes to a.txt?");
    assert_eq!(
        super::edit_question_title("a.txt", EditQuestionKind::Conflict),
        "a.txt changed on the server while you were editing"
    );
}

#[test]
fn only_irreversible_or_trusting_answers_are_risky_to_accept() {
    let trust = Modal::Question(Dialog::from_question(
        1,
        Question::TrustHostKey {
            name: "n".to_string(),
            host: "h".to_string(),
            port: 22,
            key_type: "ssh-ed25519".to_string(),
            fingerprint: "SHA256:x".to_string(),
        },
        None,
    ));

    assert!(trust.is_risky_to_accept());
    assert!(
        Modal::ConfirmDelete { side: PanelSide::Local, place: String::new(), entries: vec![entry("a")], count: None }
            .is_risky_to_accept()
    );
    assert!(Modal::ConfirmClearHistory.is_risky_to_accept());
    assert!(Modal::ConfirmQuit { transfers: true, edits: false }.is_risky_to_accept());
    assert!(!question(1).is_risky_to_accept());
    assert!(!Modal::Info { title: String::new(), body: String::new() }.is_risky_to_accept());
}

#[test]
fn delete_lines_end_with_the_contents_line_only_when_a_count_exists() {
    let entries = [entry("photos")];
    let bare = delete_lines(&entries, "this computer", None);
    let counted = DeleteCount {
        request_id: 1,
        contents: Contents::Counted(ContentCount { files: 3, ..ContentCount::default() }),
    };
    let with_count = delete_lines(&entries, "this computer", Some(&counted));

    assert_eq!(with_count.len(), bare.len() + 1);
    assert_eq!(with_count[..bare.len()], bare[..]);
    assert_eq!(with_count.last().unwrap(), "Inside: 3 files.");
}

#[test]
fn delete_lines_say_so_while_counting_and_when_counting_failed() {
    let entries = [entry("photos")];
    let counting = DeleteCount { request_id: 1, contents: Contents::Counting };
    let unavailable = DeleteCount { request_id: 2, contents: Contents::Unavailable };

    assert!(delete_lines(&entries, "x", Some(&counting)).last().unwrap().starts_with("Counting"));
    assert!(delete_lines(&entries, "x", Some(&unavailable)).last().unwrap().starts_with("Couldn't count"));
}
