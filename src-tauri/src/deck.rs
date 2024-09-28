use std::collections::HashMap;

use ap_core::cards::CardsDatabase;
use ap_core::models::deck::Deck;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::card::{Card, CardType};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct DeckDisplayRecord {
    pub archetype: String,
    pub main_deck: HashMap<CardType, Vec<Card>>,
    pub sideboard: Vec<Card>,
}

impl DeckDisplayRecord {
    pub fn from_decklist(value: &Deck, cards_db: &CardsDatabase) -> Self {
        let archetype = "Unknown".to_string();

        let main_quantities = value.quantities();
        let sideboard_quantities = value.sideboard_quantities();

        let mut main_cards = main_quantities
            .keys()
            .map(|card_id| {
                let mut card: Card = cards_db
                    .get(&card_id)
                    .map(|db_entry| db_entry.into())
                    .unwrap_or_else(|| {
                        let mut card = Card::default();
                        card.name = card_id.to_string();
                        card
                    });
                card.quantity = *main_quantities.get(card_id).unwrap_or(&0u16);
                card
            })
            .fold(
                HashMap::new(),
                |mut acc: HashMap<CardType, Vec<Card>>, card| {
                    let card_type = card.card_type.clone();
                    acc.entry(card_type).or_default().push(card);
                    acc
                },
            );
        main_cards.values_mut().for_each(|cards| cards.sort());

        let sideboard_cards = sideboard_quantities
            .keys()
            .copied()
            .map(|card_id| {
                let mut card: Card = cards_db
                    .get(&card_id)
                    .map(|db_entry| db_entry.into())
                    .unwrap_or_else(|| {
                        let mut card = Card::default();
                        card.name = card_id.to_string();
                        card
                    });
                card.quantity = *sideboard_quantities.get(&card_id).unwrap_or(&0u16);
                card
            })
            .sorted()
            .collect();

        Self {
            archetype,
            main_deck: main_cards,
            sideboard: sideboard_cards,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckDifference {
    pub added: Vec<Card>,
    pub removed: Vec<Card>,
}

impl DeckDifference {
    pub fn new(added: Vec<Card>, removed: Vec<Card>) -> Self {
        Self { added, removed }
    }
    fn missing_cards(main1: &HashMap<i32, u16>, main2: &HashMap<i32, u16>) -> HashMap<i32, u16> {
        let mut missing = Vec::new();
        for (card_id, quantity) in main1 {
            if let Some(deck2_quantity) = main2.get(card_id) {
                if deck2_quantity < quantity {
                    let diff = quantity - deck2_quantity;
                    (0..diff).for_each(|_| missing.push(*card_id));
                }
            } else {
                (0u16..*quantity).for_each(|_| missing.push(*card_id));
            }
        }
        quantities(&missing)
    }

    fn aggregate(collection: &HashMap<i32, u16>, cards_database: &CardsDatabase) -> Vec<Card> {
        collection
            .iter()
            .map(|(card_id, quantity)| {
                let mut card: Card = cards_database
                    .get(&card_id)
                    .map(|db_entry| db_entry.into())
                    .unwrap_or_else(|| {
                        let mut card = Card::default();
                        card.name = card_id.to_string();
                        card
                    });
                card.quantity = *quantity;
                card
            })
            .sorted()
            .collect()
    }
    pub fn difference(deck1: &Deck, deck2: &Deck, cards_database: &CardsDatabase) -> Self {
        let deck1_quantities = deck1.quantities();
        let deck2_quantities = deck2.quantities();

        let added = Self::missing_cards(&deck2_quantities, &deck1_quantities);
        let removed = Self::missing_cards(&deck1_quantities, &deck2_quantities);

        let added = Self::aggregate(&added, cards_database);
        let removed = Self::aggregate(&removed, cards_database);

        Self::new(added, removed)
    }
}

fn quantities(deck: &[i32]) -> HashMap<i32, u16> {
    let unique: Vec<_> = deck.iter().unique().copied().collect();
    let deck_quantities: HashMap<i32, u16> = unique
        .iter()
        .map(|ent_id| {
            let quantity =
                u16::try_from(deck.iter().filter(|&id| id == ent_id).count()).unwrap_or_default();
            (*ent_id, quantity)
        })
        .collect();
    deck_quantities
}
