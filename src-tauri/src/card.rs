use std::{cmp::Ordering, fmt::Display};

use ap_core::cards::CardDbEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardType {
    Creature,
    Land,
    Artifact,
    Enchantment,
    Planeswalker,
    Instant,
    Sorcery,
    Battle,
    #[default]
    Unknown,
}

impl Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        serde_json::to_string(self)
            .unwrap_or("Unknown".to_string())
            .fmt(f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Card {
    pub name: String,
    pub type_field: CardType,
    pub mana_value: i16,
    pub quantity: u16,
    pub image_uri: String,
}

impl Card {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }
}

impl Default for Card {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            type_field: CardType::Unknown,
            mana_value: 0,
            quantity: 0,
            image_uri: String::new(),
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
                .and_then(|faces| faces.first().map(|face| face.image_uri.clone()))
                .flatten()
                .as_ref()
                .unwrap_or(&String::new())
                .to_string()
        };

        Self {
            name,
            type_field: card_type_from_type_line(&type_line),
            #[allow(clippy::cast_possible_truncation)]
            mana_value: entry.cmc as i16,
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
