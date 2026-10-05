// SPDX-License-Identifier: MIT
//! Local projection only: no new IPC, store writes or selection side effects.
use crate::model::{Snapshot, display};

pub enum Row {
    Group { name: Option<String>, count: usize },
    Profile(usize),
}

pub fn visible(snapshot: &Snapshot, query: &str, favorites: bool) -> Vec<usize> {
    let query = query.to_lowercase();
    let mut indices: Vec<_> = snapshot
        .metadata
        .profiles
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            (!favorites || p.favorite)
                && (display(&p.name, 80).to_lowercase().contains(&query)
                    || p.subscription_id
                        .as_ref()
                        .and_then(|id| snapshot.metadata.subscriptions.iter().find(|s| s.id == *id))
                        .is_some_and(|s| display(&s.name, 80).to_lowercase().contains(&query)))
        })
        .map(|(i, _)| i)
        .collect();
    // Local first; then canonical subscription order. Preserve feed order inside
    // a group and keep record identity separate from duplicate display names.
    indices.sort_by_key(|i| group(snapshot, *i));
    indices
}
fn group(snapshot: &Snapshot, i: usize) -> usize {
    snapshot.metadata.profiles[i]
        .subscription_id
        .as_ref()
        .and_then(|id| {
            snapshot
                .metadata
                .subscriptions
                .iter()
                .position(|s| s.id == *id)
        })
        .map_or(0, |i| i + 1)
}

pub fn rows(snapshot: &Snapshot, indices: &[usize]) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut offset = 0;
    while offset < indices.len() {
        let key = group(snapshot, indices[offset]);
        let count = indices[offset..]
            .iter()
            .take_while(|i| group(snapshot, **i) == key)
            .count();
        let name = key
            .checked_sub(1)
            .map(|i| snapshot.metadata.subscriptions[i].name.clone());
        rows.push(Row::Group { name, count });
        rows.extend(
            indices[offset..offset + count]
                .iter()
                .map(|i| Row::Profile(*i)),
        );
        offset += count;
    }
    rows
}
