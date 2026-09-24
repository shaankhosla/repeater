use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::cloze_utils::mask_cloze_text;
use crate::llm::drill_preprocessor::AIStatus;

#[derive(Clone, Debug)]
pub struct Card {
    pub file_path: PathBuf,
    #[allow(dead_code)]
    pub file_card_range: (usize, usize),
    pub content: CardContent,
    pub card_hash: String,
    pub ai_status: AIStatus,
}

impl Card {
    pub fn new(
        file_path: PathBuf,
        file_card_range: (usize, usize),
        content: CardContent,
        card_hash: String,
    ) -> Self {
        Card {
            file_path,
            file_card_range,
            content,
            card_hash,
            ai_status: AIStatus::NoNeed,
        }
    }

    pub fn presentation(&self) -> CardPresentation<'_> {
        match &self.content {
            CardContent::Basic { question, answer } => CardPresentation {
                kind: CardType::Basic,
                question: Cow::Borrowed(question),
                answer,
            },
            CardContent::Cloze { text, cloze_range } => CardPresentation {
                kind: CardType::Cloze,
                question: cloze_range.as_ref().map_or_else(
                    || Cow::Borrowed(text.as_str()),
                    |range| Cow::Owned(mask_cloze_text(text, range)),
                ),
                answer: text,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum CardContent {
    Basic {
        question: String,
        answer: String,
    },
    Cloze {
        text: String,
        cloze_range: Option<ClozeRange>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardPresentation<'a> {
    pub kind: CardType,
    pub question: Cow<'a, str>,
    pub answer: &'a str,
}

#[derive(Clone, Debug)]
pub struct ClozeRange {
    pub start: usize,
    pub end: usize,
}

impl ClozeRange {
    pub fn new(start: usize, end: usize) -> Result<Self> {
        if start >= end {
            bail!("Invalid cloze range: start must be < end");
        }

        if end - start <= 2 {
            bail!("Invalid cloze range: range must be at least length 1");
        }

        Ok(Self { start, end })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardType {
    Basic,
    Cloze,
}
