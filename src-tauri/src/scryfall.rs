use std::cmp::Ordering;
use crate::deck::CardType;
use ap_core::cards::CardsDatabase;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Card {
    pub name: String,
    pub mana_value: u16,
    pub quantity: u16,
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
pub fn mana_value_from_mana_cost(mana_cost: &str) -> u16 {
    let re = Regex::new(r"\{(\d+)\}").unwrap();
    let mut total_mana_value = 0;
    for cap in re.captures_iter(mana_cost) {
        let mana_value = cap[1].parse::<u16>().unwrap_or(1);
        total_mana_value += mana_value;
    }
    total_mana_value
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

    fn get_cached_card(&self, card_id: i32) -> anyhow::Result<Option<(CardType, Card)>> {
        let mut statement = self
            .conn
            .prepare("SELECT card_type, card_json FROM cards WHERE id = ?1")?;
        let mut rows = statement.query(&[&card_id])?;
        let row = rows.next()?;
        match row {
            Some(row) => {
                let card_type_str: String = row.get(0)?;
                let card_json: String = row.get(1)?;
                let card: Card = serde_json::from_str(&card_json)?;
                let card_type: CardType = serde_json::from_str(&card_type_str)?;
                Ok(Some((card_type, card)))
            }
            None => Ok(None),
        }
    }

    fn save_card(&self, card_id: i32, card_type: CardType, card: &Card) -> anyhow::Result<()> {
        let card_json = serde_json::to_string(card).unwrap();
        let result = self.conn.execute(
            "INSERT INTO cards (id, card_type, card_json) VALUES (?1, ?2, ?3)",
            (card_id, &card_type.to_string(), &card_json),
        );
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                info!("Error saving card: {:?}", e);
                Ok(())
            }
        }
    }

    pub fn get_card_info(
        &self,
        card_id: i32,
        cards_database: &CardsDatabase,
    ) -> anyhow::Result<(CardType, Card)> {
        let pretty_name = cards_database.get_pretty_name(&card_id.to_string());
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

        let cached = self.get_cached_card(swapped_card_id)?;
        match cached {
            Some((card_type, card)) => Ok((card_type, card)),
            None => {
                let (card_type, card) = self.fetch_card_info(swapped_card_id)?;
                info!("Fetched card info for card_id: {}", card_id);
                self.save_card(swapped_card_id, card_type.clone(), &card)?;
                Ok((card_type, card))
            }
        }
    }

    fn fetch_card_info(&self, card_id: i32) -> anyhow::Result<(CardType, Card)> {
        let response = self
            .client
            .get(format!("https://api.scryfall.com/cards/arena/{}", card_id))
            .send()?;
        let resp_json: Value = response.json()?;
        let mana_value = resp_json["cmc"].as_f64().unwrap_or(0.0) as u16;
        let card_id_str = card_id.to_string();

        let card = Card {
            name: resp_json["name"]
                .as_str()
                .unwrap_or(&card_id_str)
                .to_string(),
            mana_value,
            quantity: 1,
        };
        let card_type = if resp_json["layout"].as_str().is_some() {
            resp_json["card_faces"][0]["type_line"]
                .as_str()
                .unwrap_or("")
        } else {
            resp_json["type_line"].as_str().unwrap_or("")
        };
        Ok((card_type_from_type_line(card_type), card))
    }
}
