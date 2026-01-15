use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::llm::drill_preprocessor::AIStatus;
use crate::parser::{Media, extract_media};

#[derive(Clone, Debug)]
pub struct Card {
    pub file_path: PathBuf,
    #[allow(dead_code)]
    pub file_card_range: (usize, usize),
    pub content: CardContent,
    pub card_hash: String,
    media: Vec<Media>,
    media_parsed_already: bool,
    pub ai_status: AIStatus,
}

impl Card {
    pub fn new(
        file_path: PathBuf,
        file_card_range: (usize, usize),
        content: CardContent,
        card_hash: String,
    ) -> Self {
        let media = Vec::new();
        let media_parsed_already = false;
        Card {
            file_path,
            file_card_range,
            content,
            card_hash,
            media,
            media_parsed_already,
            ai_status: AIStatus::NoNeed,
        }
    }

    pub fn parse_media(&mut self) -> Vec<Media> {
        if self.media_parsed_already {
            return self.media.clone();
        }
        self.media_parsed_already = true;
        let string_content = match &self.content {
            CardContent::Basic { question, answer } => format!("{} {}", question, answer),
            CardContent::Cloze {
                text,
                cloze_range: _,
            } => text.clone(),
        };
        self.media = extract_media(&string_content, self.file_path.parent());
        self.media.clone()
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

#[derive(Debug, PartialEq, Eq)]
pub enum CardType {
    Basic,
    Cloze,
}
