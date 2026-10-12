//! Immutable metadata projection cache, independent of status/source epochs.
use crate::{Profile, metadata::Metadata};
use std::{collections::HashSet, rc::Rc};

#[derive(Default)]
pub struct ProfileCache {
    key: Option<(String, bool, String, u64)>,
    pub visible: Rc<Vec<Profile>>,
    pub ids: HashSet<String>,
}

impl ProfileCache {
    pub fn refresh(
        &mut self,
        metadata: &Metadata,
        query: &str,
        large: bool,
        collection: &str,
    ) -> bool {
        let key = (
            query.to_owned(),
            large,
            collection.to_owned(),
            metadata.content,
        );
        if self.key.as_ref() == Some(&key) {
            return false;
        }
        let needle = query.to_lowercase();
        self.visible = Rc::new(
            metadata
                .rows
                .iter()
                .filter(|profile| {
                    let in_collection = match collection {
                        "all" => true,
                        "sample" => profile.subscription == "Sample collection",
                        "generated" => profile.subscription == "Generated samples",
                        "local" => profile.subscription.is_empty(),
                        _ => false,
                    };
                    (large || profile.subscription != "Generated samples")
                        && in_collection
                        && [
                            &profile.name,
                            &profile.country,
                            &profile.host,
                            &profile.subscription,
                        ]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&needle))
                })
                .cloned()
                .collect(),
        );
        self.ids = self
            .visible
            .iter()
            .map(|profile| profile.id.clone())
            .collect();
        self.key = Some(key);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Fixtures, workload::UpdateView};

    #[test]
    fn large_metadata_projection_is_reused_through_status_updates() {
        let fixture = Fixtures::load();
        let metadata = Metadata::fixture(&fixture);
        let mut cache = ProfileCache::default();
        assert!(cache.refresh(&metadata, "", true, "all"));
        let visible = cache.visible.clone();
        assert_eq!(visible.len(), 10006);
        let selected = "generated-09999";
        let mut view = UpdateView::default();
        view.attach();
        for _ in 0..100 {
            assert!(view.burst("connected", Some("south"), 1000));
            assert!(!cache.refresh(&metadata, "", true, "all"));
            assert!(Rc::ptr_eq(&cache.visible, &visible));
            assert!(cache.ids.contains(selected));
        }
        assert_eq!(view.state.sample, 100000);
        assert!(cache.refresh(&metadata, "no-matches", true, "all"));
        assert!(cache.visible.is_empty());
        assert_eq!(fixture.resolved_profile(selected).unwrap().id, selected);
        assert!(cache.refresh(&metadata, "", false, "all"));
        assert_eq!(cache.visible.len(), 6);
        assert!(cache.refresh(&metadata, "", true, "generated"));
        assert_eq!(cache.visible.len(), 10000);
    }
}
