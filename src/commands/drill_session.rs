use std::error::Error;
use std::fmt::{self, Display};
use std::path::PathBuf;

use chrono::Utc;
use rand::RngExt as _;
use serde::Serialize;

use crate::card::CardType;
use crate::crud::DB;
use crate::crud::drill_sessions::{
    DrillReview, DrillSession, DrillSessionSource, MarkReviewResult, NewDrillReview,
    NewDrillSession,
};
use crate::fsrs::ReviewStatus;
use crate::llm::drill_preprocessor::DrillPreprocessor;
use crate::notes::register_apple_notes_cards;
use crate::parser::register_all_cards;
use crate::utils::strip_controls_and_escapes;

use super::drill::validate_retention;

const SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Debug)]
pub struct StartOptions {
    pub paths: Vec<PathBuf>,
    pub apple_notes: bool,
    pub retention: f32,
    pub rephrase_questions: bool,
    pub shuffle: bool,
}

#[derive(Debug)]
pub struct DrillSessionError {
    code: &'static str,
    message: String,
}

impl DrillSessionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn internal(error: impl Display) -> Self {
        Self::new(
            "internal_error",
            strip_controls_and_escapes(&error.to_string()),
        )
    }

    pub fn json(&self) -> String {
        serde_json::to_string(&ErrorResponse {
            schema_version: SCHEMA_VERSION,
            error: ErrorBody {
                code: self.code,
                message: &self.message,
            },
        })
        .unwrap_or_else(|_| {
            r#"{"schema_version":1,"error":{"code":"internal_error","message":"failed to serialize error"}}"#
                .to_string()
        })
    }
}

impl Display for DrillSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DrillSessionError {}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    schema_version: u8,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct StartResponse {
    schema_version: u8,
    session_id: String,
    state: &'static str,
    expires_at: String,
}

#[derive(Serialize)]
struct NextResponse {
    schema_version: u8,
    session_id: String,
    state: &'static str,
    review: Option<QuestionResponse>,
}

#[derive(Serialize)]
struct QuestionResponse {
    review_id: String,
    card_id: String,
    kind: String,
    question: String,
    source: String,
}

#[derive(Serialize)]
struct RevealResponse {
    schema_version: u8,
    session_id: String,
    state: &'static str,
    review: AnswerResponse,
}

#[derive(Serialize)]
struct AnswerResponse {
    review_id: String,
    answer: String,
}

#[derive(Serialize)]
struct MarkResponse {
    schema_version: u8,
    session_id: String,
    state: &'static str,
    review: MarkedReviewResponse,
}

#[derive(Serialize)]
struct MarkedReviewResponse {
    review_id: String,
    result: String,
    review_count: i64,
    interval_days: i64,
    due_at: String,
}

pub async fn start(db: &DB, options: StartOptions) -> Result<(), DrillSessionError> {
    validate_retention(options.retention)
        .map_err(|error| DrillSessionError::new("invalid_retention", error.to_string()))?;

    let source = if options.apple_notes {
        DrillSessionSource::AppleNotes
    } else {
        DrillSessionSource::Paths(canonicalize_paths(options.paths)?)
    };
    let session = db
        .create_drill_session(NewDrillSession {
            session_id: new_token(),
            source,
            retention: options.retention,
            rephrase_questions: options.rephrase_questions,
            shuffle: options.shuffle,
        })
        .await
        .map_err(DrillSessionError::internal)?;

    print_json(&StartResponse {
        schema_version: SCHEMA_VERSION,
        session_id: session.session_id,
        state: "active",
        expires_at: session.expires_at.to_rfc3339(),
    })
}

pub async fn next(db: &DB, session_id: &str) -> Result<(), DrillSessionError> {
    let session = load_session(db, session_id).await?;
    if session.state == "complete" {
        return print_complete(session.session_id);
    }
    ensure_session_active(&session)?;

    if let Some(review) = db
        .get_open_drill_review(session_id)
        .await
        .map_err(DrillSessionError::internal)?
    {
        return match review.state.as_str() {
            "presented" => print_question(review),
            "revealed" => Err(DrillSessionError::new(
                "review_pending_mark",
                format!(
                    "Review {} must be marked before requesting another card.",
                    review.review_id
                ),
            )),
            _ => Err(DrillSessionError::new(
                "invalid_review_state",
                format!(
                    "Review {} has invalid state {}.",
                    review.review_id, review.state
                ),
            )),
        };
    }

    db.touch_drill_session(session_id)
        .await
        .map_err(DrillSessionError::internal)?;
    let (hash_cards, _) = match &session.source {
        DrillSessionSource::Paths(paths) => register_all_cards(db, paths.clone()).await,
        DrillSessionSource::AppleNotes => register_apple_notes_cards(db).await,
    }
    .map_err(|error| {
        DrillSessionError::new(
            "source_unavailable",
            strip_controls_and_escapes(&error.to_string()),
        )
    })?;

    let mut due_cards = db
        .due_today(&hash_cards, None, None)
        .await
        .map_err(DrillSessionError::internal)?;
    if session.shuffle {
        use rand::seq::SliceRandom;
        due_cards.shuffle(&mut rand::rng());
    }

    let Some(mut card) = due_cards.into_iter().next() else {
        db.complete_drill_session(session_id)
            .await
            .map_err(DrillSessionError::internal)?;
        return print_complete(session.session_id);
    };

    let preprocessor = DrillPreprocessor::new_noninteractive(
        std::slice::from_ref(&card),
        session.rephrase_questions,
    )
    .await
    .map_err(|error| {
        DrillSessionError::new(
            "llm_unavailable",
            strip_controls_and_escapes(&error.to_string()),
        )
    })?;
    preprocessor.initialize_card_status(std::slice::from_mut(&mut card));
    preprocessor
        .preprocess_cards(std::slice::from_mut(&mut card))
        .await
        .map_err(|error| {
            DrillSessionError::new(
                "llm_unavailable",
                strip_controls_and_escapes(&error.to_string()),
            )
        })?;

    let presentation = card.presentation();
    let card_kind = match presentation.kind {
        CardType::Basic => "basic",
        CardType::Cloze => "cloze",
    };
    let question = presentation.question.into_owned();
    let answer = presentation.answer.to_owned();
    let source_path = card.file_path.display().to_string();
    let card_hash = card.card_hash;
    let review = db
        .create_presented_drill_review(NewDrillReview {
            review_id: new_token(),
            session_id: session.session_id,
            card_hash,
            card_kind,
            question,
            answer,
            source_path,
        })
        .await
        .map_err(DrillSessionError::internal)?;

    if review.state == "revealed" {
        return Err(DrillSessionError::new(
            "review_pending_mark",
            format!(
                "Review {} must be marked before requesting another card.",
                review.review_id
            ),
        ));
    }
    print_question(review)
}

pub async fn reveal(db: &DB, review_id: &str) -> Result<(), DrillSessionError> {
    let review = db
        .get_drill_review(review_id)
        .await
        .map_err(DrillSessionError::internal)?
        .ok_or_else(|| {
            DrillSessionError::new(
                "review_not_found",
                format!("Review {review_id} does not exist."),
            )
        })?;
    let session = load_session(db, &review.session_id).await?;
    ensure_session_active_or_complete(&session)?;

    let review = db
        .reveal_drill_review(review_id)
        .await
        .map_err(DrillSessionError::internal)?
        .ok_or_else(|| {
            DrillSessionError::new(
                "review_not_found",
                format!("Review {review_id} does not exist."),
            )
        })?;

    print_json(&RevealResponse {
        schema_version: SCHEMA_VERSION,
        session_id: review.session_id,
        state: if review.state == "marked" {
            "marked"
        } else {
            "awaiting_mark"
        },
        review: AnswerResponse {
            review_id: review.review_id,
            answer: review.answer,
        },
    })
}

pub async fn mark(
    db: &DB,
    review_id: &str,
    review_status: ReviewStatus,
) -> Result<(), DrillSessionError> {
    let result = db
        .mark_drill_review(review_id, review_status)
        .await
        .map_err(DrillSessionError::internal)?;

    match result {
        MarkReviewResult::Applied(review) | MarkReviewResult::AlreadyApplied(review) => {
            print_marked(review)
        }
        MarkReviewResult::NotFound => Err(DrillSessionError::new(
            "review_not_found",
            format!("Review {review_id} does not exist."),
        )),
        MarkReviewResult::NotRevealed => Err(DrillSessionError::new(
            "review_not_revealed",
            format!("Review {review_id} must be revealed before it can be marked."),
        )),
        MarkReviewResult::ConflictingResult => Err(DrillSessionError::new(
            "conflicting_review_result",
            format!("Review {review_id} was already marked with a different result."),
        )),
        MarkReviewResult::SessionExpired => Err(DrillSessionError::new(
            "session_expired",
            format!("The session containing review {review_id} has expired."),
        )),
    }
}

async fn load_session(db: &DB, session_id: &str) -> Result<DrillSession, DrillSessionError> {
    db.get_drill_session(session_id)
        .await
        .map_err(DrillSessionError::internal)?
        .ok_or_else(|| {
            DrillSessionError::new(
                "session_not_found",
                format!("Session {session_id} does not exist."),
            )
        })
}

fn ensure_session_active(session: &DrillSession) -> Result<(), DrillSessionError> {
    if session.state == "expired" || session.expires_at <= Utc::now() {
        return Err(DrillSessionError::new(
            "session_expired",
            format!("Session {} has expired.", session.session_id),
        ));
    }
    if session.state != "active" {
        return Err(DrillSessionError::new(
            "invalid_session_state",
            format!(
                "Session {} has invalid state {}.",
                session.session_id, session.state
            ),
        ));
    }
    Ok(())
}

fn ensure_session_active_or_complete(session: &DrillSession) -> Result<(), DrillSessionError> {
    if session.state == "complete" {
        return Ok(());
    }
    ensure_session_active(session)
}

fn canonicalize_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, DrillSessionError> {
    paths
        .into_iter()
        .map(|path| {
            path.canonicalize().map_err(|error| {
                DrillSessionError::new(
                    "source_unavailable",
                    format!("Cannot resolve {}: {error}", path.display()),
                )
            })
        })
        .collect()
}

fn new_token() -> String {
    format!("{:032x}", rand::rng().random::<u128>())
}

fn print_complete(session_id: String) -> Result<(), DrillSessionError> {
    print_json(&NextResponse {
        schema_version: SCHEMA_VERSION,
        session_id,
        state: "complete",
        review: None,
    })
}

fn print_question(review: DrillReview) -> Result<(), DrillSessionError> {
    print_json(&NextResponse {
        schema_version: SCHEMA_VERSION,
        session_id: review.session_id,
        state: "awaiting_reveal",
        review: Some(QuestionResponse {
            review_id: review.review_id,
            card_id: review.card_hash,
            kind: review.card_kind,
            question: review.question,
            source: review.source_path,
        }),
    })
}

fn print_marked(review: DrillReview) -> Result<(), DrillSessionError> {
    print_json(&MarkResponse {
        schema_version: SCHEMA_VERSION,
        session_id: review.session_id,
        state: "marked",
        review: MarkedReviewResponse {
            review_id: review.review_id,
            result: review.review_result.ok_or_else(|| {
                DrillSessionError::new("invalid_review_state", "Marked review has no result.")
            })?,
            review_count: review.resulting_review_count.ok_or_else(|| {
                DrillSessionError::new(
                    "invalid_review_state",
                    "Marked review has no resulting review count.",
                )
            })?,
            interval_days: review.interval_days.ok_or_else(|| {
                DrillSessionError::new(
                    "invalid_review_state",
                    "Marked review has no resulting interval.",
                )
            })?,
            due_at: review.due_at.ok_or_else(|| {
                DrillSessionError::new(
                    "invalid_review_state",
                    "Marked review has no resulting due date.",
                )
            })?,
        },
    })
}

fn print_json<T: Serialize>(value: &T) -> Result<(), DrillSessionError> {
    let json = serde_json::to_string(value).map_err(DrillSessionError::internal)?;
    println!("{json}");
    Ok(())
}
