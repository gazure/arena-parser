use std::collections::HashMap;
use std::fmt::Display;

use ap_core::cards::CardsDatabase;
use ap_core::deck::Deck;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

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
        serde_json::to_string(self)
            .unwrap_or("Unknown".to_string())
            .fmt(f)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct GoldfishDeckDisplayRecord {
    pub archetype: String,
    pub main_deck: HashMap<CardType, Vec<Card>>,
    pub sideboard: Vec<Card>,
}

impl GoldfishDeckDisplayRecord {
    pub fn from_decklist(
        value: Deck,
        scryfall: &ScryfallDataManager,
        cards_db: &CardsDatabase,
    ) -> Self {
        let archetype = "Unknown".to_string();

        let main_quantities = value.quantities();
        let sideboard_quantities = value.sideboard_quantities();

        let mut main_cards = main_quantities
            .keys()
            .map(|card_id| {
                let mut card = scryfall.get_card_info(*card_id, cards_db);
                card.quantity = main_quantities.get(card_id).unwrap_or(&0).clone();
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
            .filter_map(|card_id| {
                let mut card = scryfall.get_card_info(card_id, cards_db);
                card.quantity = sideboard_quantities.get(&card_id).unwrap_or(&0).clone();
                Some(card)
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
        for (card_id, quantity) in main1.iter() {
            if !main2.contains_key(card_id) {
                (0u16..*quantity).for_each(|_| missing.push(*card_id));
            } else {
                let deck2_quantity = main2.get(card_id).unwrap();
                if deck2_quantity < quantity {
                    let diff = quantity - deck2_quantity;
                    (0..diff).for_each(|_| missing.push(*card_id));
                }
            }
        }
        quantities(&missing)
    }

    fn aggregate(
        collection: &HashMap<i32, u16>,
        scryfall: &ScryfallDataManager,
        cards_database: &CardsDatabase,
    ) -> Vec<Card> {
        collection
            .iter()
            .filter_map(|(card_id, quantity)| {
                let mut card = scryfall.get_card_info(*card_id, cards_database);
                card.quantity = *quantity;
                Some(card)
            })
            .sorted()
            .collect()
    }
    pub fn difference(
        deck1: &Deck,
        deck2: &Deck,
        scryfall: &ScryfallDataManager,
        cards_database: &CardsDatabase,
    ) -> Self {
        let deck1_quantities = deck1.quantities();
        let deck2_quantities = deck2.quantities();

        let added = Self::missing_cards(&deck2_quantities, &deck1_quantities);
        let removed = Self::missing_cards(&deck1_quantities, &deck2_quantities);

        let added = Self::aggregate(&added, scryfall, cards_database);
        let removed = Self::aggregate(&removed, scryfall, cards_database);

        Self::new(added, removed)
    }
}

fn quantities(deck: &[i32]) -> HashMap<i32, u16> {
    let unique: Vec<_> = deck.iter().unique().cloned().collect();
    let deck_quantities: HashMap<i32, u16> = unique
        .iter()
        .map(|ent_id| {
            let quantity = deck.iter().filter(|&id| id == ent_id).count() as u16;
            (*ent_id, quantity)
        })
        .collect();
    deck_quantities
}
