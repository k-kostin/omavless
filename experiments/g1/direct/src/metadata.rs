//! Synthetic bounded metadata replacements, not a product projection/parser.
use crate::{Fixtures, Profile, generated_profile};
use serde::Deserialize;
use std::{collections::HashMap, rc::Rc};

// G1a research bound, deliberately distinct from the product's 256 profiles.
pub const MAX_ROWS: usize = 10006;

#[derive(Clone, Default)]
pub struct Metadata {
    pub rows: Rc<Vec<Profile>>,
    pub content: u64,
    index: Rc<HashMap<String, usize>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplacementFixture {
    renamed_prefix: String,
    replacement_profiles: Vec<Profile>,
}

impl Metadata {
    pub fn validate(rows: Vec<Profile>, previous: &Self) -> Option<Self> {
        if rows.len() > MAX_ROWS {
            return None;
        }
        let mut index = HashMap::with_capacity(rows.len());
        for (position, row) in rows.iter().enumerate() {
            if !valid_id(&row.id)
                || index.insert(row.id.clone(), position).is_some()
                || [&row.name, &row.country, &row.host, &row.subscription]
                    .iter()
                    .any(|text| {
                        text.len() > 512 || text.chars().count() > 160 || text.chars().any(|c| {
                            c.is_control()
                                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{061c}')
                        })
                    })
                || row.name.trim().is_empty()
                || !row.host.ends_with(".example")
                || !matches!(
                    row.subscription.as_str(),
                    "" | "Sample collection" | "Generated samples"
                )
            {
                return None;
            }
        }
        if rows == *previous.rows {
            return Some(previous.clone());
        }
        Some(Self {
            rows: Rc::new(rows),
            content: previous.content.checked_add(1)?,
            index: Rc::new(index),
        })
    }

    pub fn fixture(fixture: &Fixtures) -> Self {
        let mut rows = fixture.profiles.clone();
        rows.extend((0..fixture.large_list_count).map(generated_profile));
        Self::validate(rows, &Self::default()).expect("bounded synthetic metadata")
    }

    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.index.get(id).map(|position| &self.rows[*position])
    }

    pub fn selection(&self, selected: Option<&str>) -> Option<String> {
        selected
            .filter(|id| self.profile(id).is_some())
            .map(str::to_owned)
    }

    pub fn renamed(&self, id: &str) -> Option<Vec<Profile>> {
        let fixture: ReplacementFixture =
            serde_json::from_str(include_str!("../../metadata-fixtures.json")).ok()?;
        let mut rows = self.rows.as_ref().clone();
        let row = rows.get_mut(*self.index.get(id)?)?;
        if !row.name.starts_with(&fixture.renamed_prefix) {
            row.name = format!("{}{}", fixture.renamed_prefix, row.name);
        }
        Some(rows)
    }

    pub fn removed(&self, id: &str) -> Option<Vec<Profile>> {
        self.profile(id)?;
        Some(
            self.rows
                .iter()
                .filter(|row| row.id != id)
                .cloned()
                .collect(),
        )
    }

    pub fn replaced_collection(&self) -> Vec<Profile> {
        let fixture: ReplacementFixture =
            serde_json::from_str(include_str!("../../metadata-fixtures.json"))
                .expect("synthetic replacement fixture");
        let mut rows: Vec<_> = self
            .rows
            .iter()
            .filter(|row| row.subscription != "Sample collection")
            .cloned()
            .collect();
        rows.extend(fixture.replacement_profiles);
        rows
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}
