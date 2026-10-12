//! Immutable fixture metadata cache, independent of status/source epochs.
use crate::{Fixtures, Profile, generated_profile};
use std::{collections::HashSet, rc::Rc};

#[derive(Default)]
pub struct ProfileCache {
    key: Option<(String, bool, usize)>,
    pub visible: Rc<Vec<Profile>>,
    pub ids: HashSet<String>,
}

impl ProfileCache {
    pub fn refresh(
        &mut self,
        fixture: &Fixtures,
        query: &str,
        large: bool,
        collection: usize,
    ) -> bool {
        let key = (query.to_owned(), large, collection);
        if self.key.as_ref() == Some(&key) {
            return false;
        }
        let needle = query.to_lowercase();
        let collection_id = fixture.collections[collection].id.as_str();
        let mut profiles = fixture.profiles.clone();
        if large {
            profiles.extend((0..fixture.large_list_count).map(generated_profile));
        }
        self.visible = Rc::new(
            profiles
                .into_iter()
                .filter(|profile| {
                    let in_collection = match collection_id {
                        "all" => true,
                        "sample" => profile.subscription == "Sample collection",
                        "generated" => profile.subscription == "Generated samples",
                        "local" => profile.subscription.is_empty(),
                        _ => false,
                    };
                    in_collection
                        && [
                            &profile.name,
                            &profile.country,
                            &profile.host,
                            &profile.subscription,
                        ]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&needle))
                })
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
    use crate::workload::UpdateView;

    #[test]
    fn large_metadata_projection_is_reused_through_status_updates() {
        let fixture = Fixtures::load();
        let mut cache = ProfileCache::default();
        assert!(cache.refresh(&fixture, "", true, 0));
        let visible = cache.visible.clone();
        assert_eq!(visible.len(), 10006);
        let selected = "generated-09999";
        let mut view = UpdateView::default();
        view.attach();
        for _ in 0..100 {
            assert!(view.burst("connected", Some("south"), 1000));
            assert!(!cache.refresh(&fixture, "", true, 0));
            assert!(Rc::ptr_eq(&cache.visible, &visible));
            assert!(cache.ids.contains(selected));
        }
        assert_eq!(view.state.sample, 100000);
        assert!(cache.refresh(&fixture, "no-matches", true, 0));
        assert!(cache.visible.is_empty());
        assert_eq!(fixture.resolved_profile(selected).unwrap().id, selected);
        assert!(cache.refresh(&fixture, "", false, 0));
        assert_eq!(cache.visible.len(), 6);
        assert!(cache.refresh(&fixture, "", true, 2));
        assert_eq!(cache.visible.len(), 10000);
    }
}
