use std::collections::HashMap;
use std::fmt::Display;

use anyhow::Result;
use ap_core::cards::CardsDatabase;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::DeckList;
use crate::scryfall::{Card, ScryfallDataManager};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardType {
    Creature,
    Land,
    Artifact,
    Enchantment,
    Planeswalker,
    Instant,
    Sorcery,
    Battle,
    Unknown,
}

impl Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        serde_json::to_string(self).unwrap().fmt(f)
    }
}


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct GoldfishDeckDisplayRecord {
    pub archetype: String,
    pub main_deck: HashMap<CardType, Vec<Card>>,
    pub sideboard: Vec<Card>,
}

impl GoldfishDeckDisplayRecord {
    pub fn from_decklist(value: DeckList, scryfall: &ScryfallDataManager, cards_db: &CardsDatabase) -> Result<Self> {
        let archetype = "Unknown".to_string();

        let unique_main: Vec<_> = value.deck.iter().unique().cloned().collect();
        let main_quantities: HashMap<i32, u16> = unique_main
            .iter()
            .map(|card_id| {
                let quantity = value.deck.iter().filter(|&id| id == card_id).count() as u16;
                (*card_id, quantity)
            })
            .collect::<HashMap<i32, u16>>();
        let unique_side: Vec<_> = value.sideboard.iter().unique().cloned().collect();
        let main_cards = unique_main
            .iter()
            .map(|card_id| {
                let (card_type, mut card) = scryfall.get_card_info(*card_id, cards_db)?;
                card.quantity = main_quantities.get(card_id).unwrap_or(&0).clone();
                Ok((card_type, card))
            })
            .fold(HashMap::new(), |mut acc, res: Result<(CardType, Card)>| {
                if let Ok((card_type, card)) = res {
                    acc.entry(card_type).or_insert_with(Vec::new).push(card);
                }
                acc
            });

        let sideboard_cards = unique_side
            .iter()
            .filter_map(|card_id| {
                let (_, mut card) = scryfall.get_card_info(*card_id, cards_db).ok()?;
                card.quantity = value.sideboard.iter().filter(|&id| id == card_id).count() as u16;
                Some(card)
            })
            .collect();

        Ok(GoldfishDeckDisplayRecord {
            archetype,
            main_deck: main_cards,
            sideboard: sideboard_cards,
        })
    }
}

