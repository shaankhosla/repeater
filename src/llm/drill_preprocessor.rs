use std::sync::Arc;

use anyhow::{Context, Result};
use async_openai::Client;
use async_openai::config::OpenAIConfig;

use super::prompt_user::{ask_yn, cloze_user_prompt, rephrase_user_prompt};
use crate::card::{Card, CardContent, ClozeRange};
use crate::cloze_utils::find_cloze_ranges;

use super::{ensure_client, request_cloze};

use crate::llm::request_question_rephrase;
use std::collections::HashMap;

use crate::utils::pluralize;
use futures::stream::{self, StreamExt};

const MAX_CONCURRENT_LLM_REQUESTS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AIStatus {
    ClozeNeedDeletion,
    QuestionNeedRephrasing,
    NoNeed,
    AiEnhanced,
}

#[derive(Clone, Debug)]
pub struct DrillPreprocessor {
    client: Option<Arc<Client<OpenAIConfig>>>,
    rephrase_questions: bool,
}

impl DrillPreprocessor {
    pub fn new(cards: &[Card], rephrase_questions: bool) -> Result<Self> {
        let cards_needing_clozes = count_cards_needing_clozes(cards);
        let cards_needing_rephrase = if rephrase_questions {
            count_cards_needing_rephrase(cards)
        } else {
            0
        };

        let rephrase_prompt = if cards_needing_rephrase > 0 {
            rephrase_user_prompt(cards, cards_needing_rephrase)
        } else {
            None
        };
        let cloze_prompt = if cards_needing_clozes > 0 {
            cloze_user_prompt(cards, cards_needing_clozes)
        } else {
            None
        };
        let user_prompt = match (rephrase_prompt, cloze_prompt) {
            (Some(rephrase), Some(cloze)) => Some(format!("{rephrase}\n{cloze}")),
            (Some(rephrase), None) => Some(rephrase),
            (None, Some(cloze)) => Some(cloze),
            (None, None) => None,
        };

        let client = match user_prompt {
            Some(prompt) => {
                let error_message = match (cards_needing_rephrase, cards_needing_clozes) {
                    (0, cloze) => format!(
                        "Failed to initialize LLM client, cannot synthesize Cloze deletions for {cloze} Cloze cards without brackets"
                    ),
                    (rephrase, 0) => format!(
                        "Failed to initialize LLM client, cannot rephrase {rephrase} questions"
                    ),
                    (rephrase, cloze) => format!(
                        "Failed to initialize LLM client, cannot rephrase {rephrase} questions or synthesize Cloze deletions for {cloze} cards"
                    ),
                };
                Some(
                    ensure_client(&prompt)
                        .with_context(|| error_message)
                        .map(Arc::new)?,
                )
            }
            None => None,
        };

        if cards_needing_clozes > 0 {
            let ok = ask_yn(
                format!(
                    "Use AI to detect bracketed deletions in {}? [y/n]",
                    pluralize("cloze card", cards_needing_clozes)
                ),
                true,
            );
            if !ok {
                return Err(anyhow::anyhow!(
                    "Please run `repeater llm key --set ` to enable AI"
                ));
            }
        }

        Ok(Self {
            client,
            rephrase_questions,
        })
    }

    pub fn llm_required(&self) -> bool {
        self.client.is_some()
    }
    pub fn initialize_card_status(&self, cards: &mut [Card]) {
        for card in cards {
            if does_card_need_cloze(card) {
                card.ai_status = AIStatus::ClozeNeedDeletion;
            }

            if self.rephrase_questions && does_card_need_rephrase(card) {
                card.ai_status = AIStatus::QuestionNeedRephrasing;
            }
        }
    }

    pub async fn preprocess_cards(&self, cards: &mut [Card]) -> Result<()> {
        let Some(client) = self.client.as_ref() else {
            return Ok(());
        };
        if self.rephrase_questions {
            rephrase_basic_questions_with_client(cards, Arc::clone(client)).await?;
        }
        resolve_missing_clozes_with_client(cards, Arc::clone(client)).await?;
        Ok(())
    }
}

async fn replace_questions(
    cards: &mut [Card],
    cards_to_rephrase: Vec<(String, String, String)>,
    index_by_hash: &HashMap<String, usize>,
    client: Arc<Client<OpenAIConfig>>,
) -> Result<()> {
    let mut tasks = stream::iter(
        cards_to_rephrase
            .into_iter()
            .map(|(hash, question, answer)| {
                let client = Arc::clone(&client);
                async move {
                    let new_question = request_question_rephrase(&client, &question, &answer)
                        .await
                        .with_context(|| {
                            format!(
                                "Failed to rephrase question:\n\nQ: {}\nA: {}",
                                question, answer
                            )
                        })?;
                    Ok::<_, anyhow::Error>((hash, new_question))
                }
            }),
    )
    .buffer_unordered(MAX_CONCURRENT_LLM_REQUESTS);

    while let Some(result) = tasks.next().await {
        let (hash, rewritten) = result?;
        let Some(&idx) = index_by_hash.get(&hash) else {
            continue;
        };
        if let CardContent::Basic { question, .. } = &mut cards[idx].content {
            *question = rewritten;
        }
    }

    Ok(())
}

pub async fn rephrase_basic_questions_with_client(
    cards: &mut [Card],
    client: Arc<Client<OpenAIConfig>>,
) -> Result<()> {
    let cards_to_rephrase: Vec<_> = cards
        .iter()
        .filter_map(|card| {
            if let CardContent::Basic { question, answer } = &card.content {
                Some((card.card_hash.clone(), question.clone(), answer.clone()))
            } else {
                None
            }
        })
        .collect();

    if cards_to_rephrase.is_empty() {
        return Ok(());
    }

    let index_by_hash: HashMap<_, _> = cards
        .iter()
        .enumerate()
        .map(|(idx, card)| (card.card_hash.clone(), idx))
        .collect();

    replace_questions(cards, cards_to_rephrase, &index_by_hash, client).await?;
    Ok(())
}

async fn replace_missing_clozes(
    cards: &mut [Card],
    cards_with_no_clozes: Vec<(String, String)>,
    index_by_hash: &HashMap<String, usize>,
    client: Arc<Client<OpenAIConfig>>,
) -> Result<()> {
    let mut tasks = stream::iter(cards_with_no_clozes.into_iter().map(|(hash, text)| {
        let client = Arc::clone(&client);
        async move {
            let new_cloze_text = request_cloze(&client, &text).await.with_context(|| {
                format!("Failed to synthesize cloze text for card:\n\n{}", text)
            })?;
            Ok::<_, anyhow::Error>((hash, new_cloze_text))
        }
    }))
    .buffer_unordered(MAX_CONCURRENT_LLM_REQUESTS);
    while let Some(llm_output) = tasks.next().await {
        let (hash, new_cloze_text) = llm_output?;

        let Some(&idx) = index_by_hash.get(&hash) else {
            continue;
        };
        let card = &mut cards[idx];
        if let CardContent::Cloze {
            text, cloze_range, ..
        } = &mut card.content
        {
            let cloze_idxs = find_cloze_ranges(&new_cloze_text);
            let new_cloze_range: ClozeRange = cloze_idxs
                .first()
                .map(|(start, end)| ClozeRange::new(*start, *end))
                .transpose()?
                .ok_or_else(|| {
                    anyhow::anyhow!("No cloze range found. LLM output: {new_cloze_text}")
                })?;
            *cloze_range = Some(new_cloze_range);
            *text = new_cloze_text;
        }
    }

    Ok(())
}

pub async fn resolve_missing_clozes_with_client(
    cards: &mut [Card],
    client: Arc<Client<OpenAIConfig>>,
) -> Result<()> {
    let cards_with_no_clozes: Vec<_> = cards
        .iter()
        .filter_map(|card| {
            if let CardContent::Cloze {
                text,
                cloze_range: None,
            } = &card.content
            {
                Some((card.card_hash.clone(), text.clone()))
            } else {
                None
            }
        })
        .collect();

    if cards_with_no_clozes.is_empty() {
        return Ok(());
    }

    let index_by_hash: HashMap<String, usize> = cards
        .iter()
        .enumerate()
        .map(|(i, c)| (c.card_hash.clone(), i))
        .collect();

    replace_missing_clozes(cards, cards_with_no_clozes, &index_by_hash, client).await?;

    Ok(())
}

fn count_cards_needing_clozes(cards: &[Card]) -> usize {
    cards
        .iter()
        .filter(|card| does_card_need_cloze(card))
        .count()
}

pub fn does_card_need_cloze(card: &Card) -> bool {
    matches!(
        card.content,
        CardContent::Cloze {
            cloze_range: None,
            ..
        }
    )
}

fn count_cards_needing_rephrase(cards: &[Card]) -> usize {
    cards
        .iter()
        .filter(|card| does_card_need_rephrase(card))
        .count()
}
fn does_card_need_rephrase(card: &Card) -> bool {
    matches!(card.content, CardContent::Basic { .. })
}
