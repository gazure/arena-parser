use std::cmp::Ordering;
use std::collections::HashMap;

use ap_core::cards::{CardDbEntry, CardsDatabase};
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
    pub image_uri: String,
}

impl Card {
    pub fn new(
        name: String,
        card_type: CardType,
        mana_value: u16,
        quantity: u16,
        image_uri: String,
    ) -> Self {
        Self {
            name,
            card_type,
            mana_value,
            quantity,
            image_uri,
        }
    }
}

impl Default for Card {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            card_type: CardType::Unknown,
            mana_value: 0,
            quantity: 0,
            image_uri: "".to_string(),
        }
    }
}

impl Eq for Card {}

impl Ord for Card {
    fn cmp(&self, other: &Self) -> Ordering {
        let mana_value_ordering = self.mana_value.cmp(&other.mana_value);
        if mana_value_ordering == Ordering::Equal {
            self.name.cmp(&other.name)
        } else {
            mana_value_ordering
        }
    }
}

impl PartialEq<Self> for Card {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl PartialOrd<Self> for Card {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<&CardDbEntry> for Card {
    fn from(entry: &CardDbEntry) -> Self {
        let (name, type_line) = if let Some(card_faces) = &entry.card_faces {
            let front_face = &card_faces[0];
            (front_face.name.clone(), front_face.type_line.clone())
        } else {
            (entry.name.clone(), entry.type_line.clone())
        };
        let image_uri = if let Some(image_uri) = &entry.image_uri {
            image_uri.clone()
        } else {
            // Gross!
            entry
                .card_faces
                .as_ref()
                .map(|faces| {
                    faces
                        .get(0)
                        .map(|face| face.image_uri.as_ref().map(|uri| uri.clone()))
                })
                .flatten()
                .flatten()
                .as_ref()
                .unwrap_or(&&"".to_string())
                .to_string()
        };

        Self {
            name,
            card_type: card_type_from_type_line(&type_line),
            mana_value: entry.cmc as u16,
            quantity: 1,
            image_uri: image_uri.clone(),
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

#[allow(dead_code)]
mod scryfall {
    use super::*;

    #[derive(Debug)]
    struct ScryfallDataManager {
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
            let client = reqwest::blocking::ClientBuilder::new()
                .user_agent("ArenaParser/0.1")
                .build()
                .unwrap();

            Self {
                client,
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
            let mut rows = statement.query([&card_id])?;
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

        pub fn get_card_info(&self, card_id: i32, cards_database: &CardsDatabase) -> Card {
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
            cached.unwrap_or_else(|| match self.fetch_card_info(swapped_card_id) {
                Ok(card) => {
                    self.save_card(swapped_card_id, &card);
                    card
                }
                Err(e) => {
                    warn!("Error fetching card info: {:?}", e);
                    Card::new(card_id.to_string(), CardType::Unknown, 0, 1, "".to_string())
                }
            })
        }

        fn fetch_card_info(&self, card_id: i32) -> anyhow::Result<Card> {
            let response = self
                .client
                .get(format!("https://api.scryfall.com/cards/arena/{card_id}"))
                .send()?;
            let resp_json: Value = response.json()?;
            let mana_value = resp_json["cmc"].as_f64().unwrap_or(0.0) as u16;
            let card_id_str = card_id.to_string();
            let layout = resp_json["layout"].as_str().unwrap_or("");
            let mut card_name = resp_json["name"]
                .as_str()
                .unwrap_or(&card_id_str)
                .to_string();
            let mut card_type =
                card_type_from_type_line(resp_json["type_line"].as_str().unwrap_or(""));

            if layout == "transform" || layout == "modal_dfc" {
                if let Some(card_faces) = resp_json["card_faces"].as_array() {
                    card_name = card_faces[0]["name"]
                        .as_str()
                        .unwrap_or(&card_id_str)
                        .to_string();
                    card_type =
                        card_type_from_type_line(card_faces[0]["type_line"].as_str().unwrap_or(""));
                }
            }

            let card = Card::new(card_name, card_type, mana_value, 1, "".to_string());
            Ok(card)
        }
    }
}
