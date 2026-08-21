use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn repeater(data_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repeater"))
        .env("REPEATER_DATA_DIR", data_dir)
        .env("OPENAI_API_KEY", "")
        .args(args)
        .output()
        .expect("repeater should run")
}

fn successful_json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(
        !stdout.contains("\u{1b}["),
        "JSON must not contain ANSI escapes"
    );
    serde_json::from_str(&stdout).expect("stdout should contain one JSON value")
}

fn failed_json(output: Output) -> Value {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        !stderr.contains("\u{1b}["),
        "JSON must not contain ANSI escapes"
    );
    serde_json::from_str(&stderr).expect("stderr should contain one JSON value")
}

#[test]
fn drill_session_is_live_hidden_and_idempotent() {
    let data_dir = TempDir::new().unwrap();
    let cards_dir = TempDir::new().unwrap();

    let start = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "start", cards_dir.path().to_str().unwrap()],
    ));
    assert_eq!(start["schema_version"], 1);
    assert_eq!(start["state"], "active");
    let session_id = start["session_id"].as_str().unwrap().to_string();

    let card_path = cards_dir.path().join("geography.md");
    fs::write(&card_path, "Q: What is the capital of France?\nA: Paris\n").unwrap();

    let next = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "next", &session_id],
    ));
    assert_eq!(next["state"], "awaiting_reveal");
    assert_eq!(next["review"]["question"], "What is the capital of France?");
    assert!(next["review"].get("answer").is_none());
    assert_eq!(
        next["review"]["source"],
        card_path.canonicalize().unwrap().display().to_string()
    );
    let review_id = next["review"]["review_id"].as_str().unwrap().to_string();

    let repeated_next = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "next", &session_id],
    ));
    assert_eq!(repeated_next["review"]["review_id"], review_id);

    let early_mark = failed_json(repeater(
        data_dir.path(),
        &["drill-session", "mark", &review_id, "pass"],
    ));
    assert_eq!(early_mark["error"]["code"], "review_not_revealed");

    fs::write(
        &card_path,
        "Q: What is the capital of France?\nA: Changed after presentation\n",
    )
    .unwrap();

    let reveal = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "reveal", &review_id],
    ));
    assert_eq!(reveal["state"], "awaiting_mark");
    assert_eq!(reveal["review"]["answer"], "Paris");

    let repeated_reveal = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "reveal", &review_id],
    ));
    assert_eq!(repeated_reveal, reveal);

    let marked = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "mark", &review_id, "pass"],
    ));
    assert_eq!(marked["state"], "marked");
    assert_eq!(marked["review"]["result"], "pass");
    assert_eq!(marked["review"]["review_count"], 1);

    let repeated_mark = successful_json(repeater(
        data_dir.path(),
        &["drill-session", "mark", &review_id, "pass"],
    ));
    assert_eq!(repeated_mark, marked);

    let conflicting_mark = failed_json(repeater(
        data_dir.path(),
        &["drill-session", "mark", &review_id, "fail"],
    ));
    assert_eq!(
        conflicting_mark["error"]["code"],
        "conflicting_review_result"
    );
}

#[test]
fn empty_and_cloze_sessions_preserve_protocol_states() {
    let empty_data = TempDir::new().unwrap();
    let empty_cards = TempDir::new().unwrap();
    let start = successful_json(repeater(
        empty_data.path(),
        &[
            "drill-session",
            "start",
            empty_cards.path().to_str().unwrap(),
        ],
    ));
    let session_id = start["session_id"].as_str().unwrap();
    let complete = successful_json(repeater(
        empty_data.path(),
        &["drill-session", "next", session_id],
    ));
    assert_eq!(complete["state"], "complete");
    assert!(complete["review"].is_null());
    assert_eq!(
        successful_json(repeater(
            empty_data.path(),
            &["drill-session", "next", session_id]
        )),
        complete
    );

    let cloze_data = TempDir::new().unwrap();
    let cloze_cards = TempDir::new().unwrap();
    fs::write(
        cloze_cards.path().join("geography.md"),
        "C: The capital of France is [Paris].\n",
    )
    .unwrap();
    let start = successful_json(repeater(
        cloze_data.path(),
        &[
            "drill-session",
            "start",
            cloze_cards.path().to_str().unwrap(),
        ],
    ));
    let session_id = start["session_id"].as_str().unwrap();
    let next = successful_json(repeater(
        cloze_data.path(),
        &["drill-session", "next", session_id],
    ));
    assert_eq!(next["review"]["kind"], "cloze");
    assert_eq!(
        next["review"]["question"],
        "The capital of France is [_____]."
    );
    let review_id = next["review"]["review_id"].as_str().unwrap();
    let reveal = successful_json(repeater(
        cloze_data.path(),
        &["drill-session", "reveal", review_id],
    ));
    assert_eq!(
        reveal["review"]["answer"],
        "The capital of France is [Paris]."
    );
}

#[test]
fn parallel_sessions_may_review_the_same_card_without_lost_updates() {
    let data_dir = TempDir::new().unwrap();
    let cards_dir = TempDir::new().unwrap();
    fs::write(
        cards_dir.path().join("shared.md"),
        "Q: Shared question?\nA: Shared answer\n",
    )
    .unwrap();

    let mut review_ids = Vec::new();
    for _ in 0..2 {
        let start = successful_json(repeater(
            data_dir.path(),
            &["drill-session", "start", cards_dir.path().to_str().unwrap()],
        ));
        let session_id = start["session_id"].as_str().unwrap();
        let next = successful_json(repeater(
            data_dir.path(),
            &["drill-session", "next", session_id],
        ));
        let review_id = next["review"]["review_id"].as_str().unwrap().to_string();
        successful_json(repeater(
            data_dir.path(),
            &["drill-session", "reveal", &review_id],
        ));
        review_ids.push(review_id);
    }

    let data_path = data_dir.path().to_path_buf();
    let first_review = review_ids.remove(0);
    let first = std::thread::spawn(move || {
        successful_json(repeater(
            &data_path,
            &["drill-session", "mark", &first_review, "pass"],
        ))
    });
    let data_path = data_dir.path().to_path_buf();
    let second_review = review_ids.remove(0);
    let second = std::thread::spawn(move || {
        successful_json(repeater(
            &data_path,
            &["drill-session", "mark", &second_review, "pass"],
        ))
    });

    let mut review_counts = [
        first.join().unwrap()["review"]["review_count"]
            .as_i64()
            .unwrap(),
        second.join().unwrap()["review"]["review_count"]
            .as_i64()
            .unwrap(),
    ];
    review_counts.sort();
    assert_eq!(review_counts, [1, 2]);
}

#[test]
fn rephrase_mode_fails_as_json_without_prompting_for_credentials() {
    let data_dir = TempDir::new().unwrap();
    let cards_dir = TempDir::new().unwrap();
    fs::write(
        cards_dir.path().join("card.md"),
        "Q: Question?\nA: Answer\n",
    )
    .unwrap();
    let start = successful_json(repeater(
        data_dir.path(),
        &[
            "drill-session",
            "start",
            cards_dir.path().to_str().unwrap(),
            "--rephrase",
        ],
    ));
    let session_id = start["session_id"].as_str().unwrap();
    let failure = failed_json(repeater(
        data_dir.path(),
        &["drill-session", "next", session_id],
    ));
    assert_eq!(failure["error"]["code"], "llm_unavailable");
}
