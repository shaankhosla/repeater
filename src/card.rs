use std::path::PathBuf;

use anyhow::{Result, bail};

#[derive(Clone, Debug)]
pub struct Card {
    pub file_path: PathBuf,
    #[allow(dead_code)]
    pub file_card_range: (usize, usize),
    pub content: CardContent,
    pub card_hash: String,
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
