use std::cmp::Ordering;
use std::collections::HashMap;

use ap_core::cards::CardsDatabase;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

use crate::deck::CardType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Card {
    pub name: String,
    pub card_type: CardType,
    pub mana_value: u16,
    pub quantity: u16,
}

impl Card {
    pub fn new(name: String, card_type: CardType, mana_value: u16, quantity: u16) -> Self {
        Self {
            name,
            card_type,
            mana_value,
            quantity,
        }
    }
}

impl Eq for Card {}

impl Ord for Card {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialEq<Self> for Card {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.mana_value == other.mana_value
    }
}
impl PartialOrd<Self> for Card {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mana_value_ordering = self.mana_value.cmp(&other.mana_value);
        if mana_value_ordering != Ordering::Equal {
            Some(mana_value_ordering)
        } else {
            Some(self.name.cmp(&other.name))
        }
    }
}

pub fn card_type_from_type_line(type_line: &str) -> CardType {
    if type_line.contains("Creature") {
        CardType::Creature
    } else if type_line.contains("Land") {
        CardType::Land
    } else if type_line.contains("Artifact") {
        CardType::Artifact
    } else if type_line.contains("Enchantment") {
        CardType::Enchantment
    } else if type_line.contains("Planeswalker") {
        CardType::Planeswalker
    } else if type_line.contains("Instant") {
        CardType::Instant
    } else if type_line.contains("Sorcery") {
        CardType::Sorcery
    } else if type_line.contains("Battle") {
        CardType::Battle
    } else {
        CardType::Unknown
    }
}

#[derive(Debug)]
pub struct ScryfallDataManager {
    client: reqwest::blocking::Client,
    conn: rusqlite::Connection,
    basics: HashMap<String, i32>,
}

impl ScryfallDataManager {
    pub(crate) fn new(conn: rusqlite::Connection) -> Self {
        let mut basics = HashMap::new();
        basics.insert("Plains".to_string(), 90789);
        basics.insert("Island".to_string(), 90791);
        basics.insert("Swamp".to_string(), 90793);
        basics.insert("Mountain".to_string(), 90795);
        basics.insert("Forest".to_string(), 90797);

        Self {
            client: reqwest::blocking::Client::new(),
            conn,
            basics,
        }
    }

    pub fn init(&self) -> anyhow::Result<()> {
        // TODO: migrations?
        let mut statement = self.conn.prepare(
            "CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY,
                card_type TEXT NOT NULL,
                card_json TEXT NOT NULL
            )",
        )?;
        statement.execute([])?;
        Ok(())
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        let mut statement = self.conn.prepare("DELETE FROM cards")?;
        statement.execute([])?;
        Ok(())
    }

    fn get_cached_card(&self, card_id: i32) -> anyhow::Result<Option<Card>> {
        let mut statement = self
            .conn
            .prepare("SELECT card_json FROM cards WHERE id = ?1")?;
        let mut rows = statement.query(&[&card_id])?;
        let row = rows.next()?;
        match row {
            Some(row) => {
                let card_json: String = row.get(0)?;
                let card: Card = serde_json::from_str(&card_json)?;
                Ok(Some(card))
            }
            None => Ok(None),
        }
    }

    fn save_card(&self, card_id: i32, card: &Card) {
        let Ok(card_json) = serde_json::to_string(card) else {
            warn!("Error serializing card: {:?}", card);
            return;
        };

        let result = self.conn.execute(
            "INSERT INTO cards (id, card_type, card_json) VALUES (?1, ?2, ?3)",
            (card_id, card.card_type.to_string(), &card_json),
        );

        if result.is_err() {
            info!("Error saving card: {:?}", result);
        }
    }

    pub fn get_card_info(
        &self,
        card_id: i32,
        cards_database: &CardsDatabase,
    ) -> Card {
        let pretty_name = cards_database.get_pretty_name(&card_id.to_string());
        // TODO: clean this up if possible
        let swapped_card_id = if let Ok(pretty_name) = pretty_name {
            let basics = self.basics.get(&pretty_name);
            if let Some(basics) = basics {
                *basics
            } else {
                card_id
            }
        } else {
            card_id
        };

        let cached = self.get_cached_card(swapped_card_id).unwrap_or_else(|e| {
            warn!("Error getting cached card: {:?}", e);
            None
        });
        cached.unwrap_or_else(|| {
            match self.fetch_card_info(swapped_card_id) {
                Ok(card) => {
                    self.save_card(swapped_card_id, &card);
                    card
                }
                Err(e) => {
                    warn!("Error fetching card info: {:?}", e);
                    Card::new(card_id.to_string(), CardType::Unknown, 0, 1)
                }
            }
        })
    }

    fn fetch_card_info(&self, card_id: i32) -> anyhow::Result<Card> {
        let response = self
            .client
            .get(format!("https://api.scryfall.com/cards/arena/{}", card_id))
            .send()?;
        let resp_json: Value = response.json()?;
        let mana_value = resp_json["cmc"].as_f64().unwrap_or(0.0) as u16;
        let card_id_str = card_id.to_string();
        let layout = resp_json["layout"].as_str().unwrap_or("");
        let mut card_name = resp_json["name"].as_str().unwrap_or(&card_id_str).to_string();
        let mut card_type = card_type_from_type_line(resp_json["type_line"].as_str().unwrap_or(""));

        if layout == "transform" || layout == "modal_dfc" {
            if let Some(card_faces) = resp_json["card_faces"].as_array() {
                card_name = card_faces[0]["name"].as_str().unwrap_or(&card_id_str).to_string();
                card_type = card_type_from_type_line(card_faces[0]["type_line"].as_str().unwrap_or(""));
            }
        }

        let card = Card::new(
            card_name,
            card_type,
            mana_value,
            1,
        );
        Ok(card)
    }
}
