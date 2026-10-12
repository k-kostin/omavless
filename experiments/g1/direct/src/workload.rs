//! Finite synthetic producer; no daemon, transport, timers or credentials.
use crate::freshness::{Freshness, Snapshot, Ticket};
use crate::{
    Profile,
    metadata::{Metadata, valid_id},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub phase: String,
    pub connected: Option<String>,
    pub sample: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            phase: "unverified".into(),
            connected: None,
            sample: 0,
        }
    }
}

#[derive(Default)]
pub struct UpdateView {
    freshness: Freshness,
    pending: Option<(State, Metadata)>,
    pub state: State,
    pub metadata: Metadata,
    pub metadata_fresh: bool,
    pub generation: u64,
    admitted_metadata: Metadata,
    admitted_generation: u64,
}

impl UpdateView {
    pub fn with_metadata(metadata: Metadata) -> Self {
        Self {
            admitted_metadata: metadata.clone(),
            metadata,
            ..Self::default()
        }
    }

    pub fn attach(&mut self) {
        self.freshness.attach("synthetic-source");
        self.pending = None;
        self.state = State::default();
        self.metadata_fresh = false;
        self.admitted_metadata = self.metadata.clone();
        self.admitted_generation = 0;
    }

    #[cfg(test)]
    pub fn receive(&mut self, ticket: &Ticket, snapshot: Snapshot<'_>) -> bool {
        self.receive_metadata(ticket, snapshot, self.admitted_generation, None)
    }

    pub fn receive_metadata(
        &mut self,
        ticket: &Ticket,
        snapshot: Snapshot<'_>,
        generation: u64,
        rows: Option<Vec<Profile>>,
    ) -> bool {
        if generation == 0
            || generation > 9007199254740991
            || snapshot.revision > 9007199254740991
            || generation < self.admitted_generation
            || snapshot.connected.is_some_and(|id| !valid_id(id))
        {
            return false;
        }
        let candidate = if let Some(rows) = rows {
            let Some(candidate) = Metadata::validate(rows, &self.admitted_metadata) else {
                return false;
            };
            if generation == self.admitted_generation
                && candidate.content != self.admitted_metadata.content
            {
                return false;
            }
            candidate
        } else {
            if generation != self.admitted_generation {
                return false;
            }
            self.admitted_metadata.clone()
        };
        if !self.freshness.receive(ticket, snapshot.clone()) {
            return false;
        }
        self.admitted_generation = generation;
        self.admitted_metadata = candidate.clone();
        self.pending = Some((
            State {
                phase: match snapshot.phase {
                    "connected" | "connecting" | "switching" | "reconnecting" | "unverified"
                    | "failed" | "recovery" | "disconnected" => snapshot.phase,
                    _ => "unverified",
                }
                .into(),
                connected: if snapshot.phase == "connected" {
                    self.freshness.confirmed().map(str::to_owned)
                } else if snapshot.phase == "switching" {
                    snapshot.connected.map(str::to_owned)
                } else {
                    None
                },
                sample: snapshot.revision,
            },
            candidate,
        ));
        true
    }

    pub fn flush(&mut self) -> bool {
        let Some((pending, metadata)) = self.pending.take() else {
            return false;
        };
        self.state = pending;
        self.metadata = metadata;
        self.metadata_fresh = true;
        self.generation = self.admitted_generation;
        true
    }

    pub fn lost(&mut self) {
        self.freshness.lost();
        self.pending = None;
        self.state = State::default();
        self.metadata_fresh = false;
        self.admitted_metadata = self.metadata.clone();
        self.admitted_generation = 0;
    }

    pub fn burst(&mut self, phase: &str, connected: Option<&str>, count: u64) -> bool {
        if !(1..=1000).contains(&count) {
            return false;
        }
        let Some(ticket) = self.freshness.request() else {
            return false;
        };
        let start = self.freshness.revision().unwrap_or(0);
        for offset in 1..=count {
            let Some(revision) = start.checked_add(offset) else {
                break;
            };
            let initial = self.admitted_generation == 0;
            self.receive_metadata(
                &ticket,
                Snapshot {
                    instance: "synthetic-source",
                    revision,
                    phase,
                    connected,
                },
                if initial { 1 } else { self.admitted_generation },
                initial.then(|| self.metadata.rows.as_ref().clone()),
            );
        }
        self.flush()
    }

    pub fn available(&self) -> bool {
        self.freshness.request().is_some()
    }

    pub fn replace(&mut self, phase: &str, connected: Option<&str>, rows: Vec<Profile>) -> bool {
        let Some(ticket) = self.freshness.request() else {
            return false;
        };
        let Some(revision) = self.freshness.revision().unwrap_or(0).checked_add(1) else {
            return false;
        };
        let Some(generation) = self.admitted_generation.checked_add(1) else {
            return false;
        };
        if !self.receive_metadata(
            &ticket,
            Snapshot {
                instance: "synthetic-source",
                revision,
                phase,
                connected,
            },
            generation,
            Some(rows),
        ) {
            return false;
        }
        self.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Fixtures, profiles::ProfileCache};
    use std::rc::Rc;

    #[test]
    fn metadata_replacement_changes_whole_projection_and_never_substitutes_selection() {
        let initial = Metadata::fixture(&Fixtures::load());
        let mut view = UpdateView::with_metadata(initial.clone());
        view.attach();
        view.burst("connected", Some("south"), 1);
        let mut cache = ProfileCache::default();
        let mut selected = Some("north".to_owned());
        cache.refresh(&view.metadata, "Renamed", false, "sample");
        assert!(cache.visible.is_empty());
        let rows = view.metadata.renamed("north").unwrap();
        assert!(view.replace("connected", Some("south"), rows));
        assert_eq!(view.metadata.selection(selected.as_deref()), selected);
        assert_eq!(
            view.metadata.profile("north").unwrap().name,
            "Renamed · North · Example"
        );
        cache.refresh(&view.metadata, "Renamed", false, "sample");
        assert_eq!(cache.visible.len(), 1);
        assert!(view.replace(
            "connected",
            Some("south"),
            view.metadata.removed("north").unwrap()
        ));
        selected = view.metadata.selection(selected.as_deref());
        assert_eq!(selected, None);
        assert!(view.metadata.profile("north").is_none());
        cache.refresh(&view.metadata, "", false, "sample");
        assert_eq!(cache.visible.len(), 4);
        assert_eq!(view.state.connected.as_deref(), Some("south"));
        assert!(view.replace(
            "connected",
            Some("south"),
            view.metadata.replaced_collection()
        ));
        cache.refresh(&view.metadata, "", false, "sample");
        assert_eq!(
            cache
                .visible
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["replacement-a", "replacement-b"]
        );
        assert!(view.metadata.profile("south").is_none());
        assert_eq!(view.state.connected.as_deref(), Some("south"));
        assert_eq!(view.metadata.selection(Some("south")), None);
        assert_eq!(
            view.metadata.selection(Some("local")).as_deref(),
            Some("local")
        );
        cache.refresh(&view.metadata, "", true, "all");
        let retained = cache.visible.clone();
        let metadata = view.metadata.rows.clone();
        for _ in 0..100 {
            view.burst("connected", Some("south"), 1000);
            assert!(Rc::ptr_eq(&metadata, &view.metadata.rows));
            assert!(!cache.refresh(&view.metadata, "", true, "all"));
            assert!(Rc::ptr_eq(&retained, &cache.visible));
        }
        assert!(view.replace(
            "connected",
            Some("south"),
            view.metadata.rows.as_ref().clone()
        ));
        assert!(Rc::ptr_eq(&metadata, &view.metadata.rows));
        assert!(!cache.refresh(&view.metadata, "", true, "all"));
        assert!(view.replace("connected", Some("south"), initial.rows.as_ref().clone()));
        assert_eq!(view.metadata.selection(selected.as_deref()), None);
        assert!(view.replace(
            "connected",
            Some("south"),
            view.metadata.removed("generated-09999").unwrap()
        ));
        assert_eq!(view.metadata.selection(Some("generated-09999")), None);
        assert!(view.metadata.profile("generated-09999").is_none());
    }

    #[test]
    fn malformed_stale_and_lost_metadata_never_replace_the_latest_whole_view() {
        let initial = Metadata::fixture(&Fixtures::load());
        let mut view = UpdateView::with_metadata(initial.clone());
        view.attach();
        view.burst("connected", Some("south"), 1);
        assert!(view.replace(
            "connected",
            Some("south"),
            view.metadata.removed("generated-09999").unwrap()
        ));
        let mut too_many = initial.rows.as_ref().clone();
        too_many.push(too_many[0].clone());
        let duplicate = vec![initial.rows[0].clone(), initial.rows[0].clone()];
        let mut long = vec![initial.rows[0].clone()];
        long[0].name = "x".repeat(513);
        let mut control = vec![initial.rows[0].clone()];
        control[0].country = "\u{1b}[2J".into();
        let mut host = vec![initial.rows[0].clone()];
        host[0].host = "actual.invalid".into();
        for rows in [too_many, duplicate, long, control, host] {
            assert!(!view.replace("connected", Some("south"), rows));
        }
        assert_eq!(view.metadata.rows.len(), 10005);
        let ticket = view.freshness.request().unwrap();
        let next = view.freshness.revision().unwrap() + 1;
        let packet = Snapshot {
            instance: "synthetic-source",
            revision: next,
            phase: "connected",
            connected: Some("south"),
        };
        assert!(!view.receive_metadata(
            &ticket,
            packet.clone(),
            view.generation,
            Some(initial.rows.as_ref().clone())
        ));
        assert!(!view.receive_metadata(&ticket, packet.clone(), view.generation + 1, None));
        assert!(!view.receive_metadata(
            &ticket,
            packet.clone(),
            0,
            Some(initial.rows.as_ref().clone())
        ));
        assert!(view.receive_metadata(
            &ticket,
            packet.clone(),
            view.generation + 1,
            Some(initial.rows.as_ref().clone())
        ));
        assert!(!view.receive_metadata(
            &ticket,
            Snapshot {
                revision: next - 1,
                ..packet.clone()
            },
            view.generation + 2,
            Some(vec![])
        ));
        view.lost();
        assert!(!view.flush());
        assert!(!view.metadata_fresh);
        assert_eq!(view.state.connected, None);
        assert!(view.metadata.profile("generated-09999").is_none());
        view.attach();
        let fresh = view.freshness.request().unwrap();
        assert!(!view.receive_metadata(
            &fresh,
            Snapshot {
                revision: 1,
                ..packet.clone()
            },
            1,
            None
        ));
        assert!(!view.receive_metadata(
            &ticket,
            Snapshot {
                revision: next + 1000,
                ..packet.clone()
            },
            1,
            Some(initial.rows.as_ref().clone())
        ));
        assert!(view.receive_metadata(
            &fresh,
            Snapshot {
                revision: 1,
                ..packet
            },
            1,
            Some(view.metadata.rows.as_ref().clone())
        ));
        assert!(view.flush());
        assert!(view.metadata_fresh);
        assert!(view.metadata.profile("generated-09999").is_none());
        assert_eq!(view.state.connected.as_deref(), Some("south"));
    }

    #[test]
    fn latest_slot_refuses_regression_and_loss_clears_queued_confirmation() {
        let mut view = UpdateView::default();
        view.attach();
        let ticket = view.freshness.request().unwrap();
        assert!(view.receive_metadata(
            &ticket,
            Snapshot {
                instance: "synthetic-source",
                revision: 0,
                phase: "unverified",
                connected: None
            },
            1,
            Some(vec![])
        ));
        view.flush();
        for revision in 1..=1000 {
            assert!(view.receive(
                &ticket,
                Snapshot {
                    instance: "synthetic-source",
                    revision,
                    phase: "connected",
                    connected: Some("south")
                }
            ));
            assert_eq!(view.pending.as_ref().unwrap().0.sample, revision);
        }
        assert_eq!(view.state.sample, 0);
        assert!(!view.receive(
            &ticket,
            Snapshot {
                instance: "wrong-source",
                revision: 2000,
                phase: "connected",
                connected: Some("north")
            }
        ));
        assert!(!view.burst("connected", Some("north"), 1001));
        assert!(!view.burst("connected", Some("north"), 0));
        assert!(!view.receive(
            &ticket,
            Snapshot {
                instance: "synthetic-source",
                revision: 999,
                phase: "connected",
                connected: Some("north")
            }
        ));
        assert!(view.flush());
        assert_eq!(view.state.sample, 1000);
        assert_eq!(view.state.connected.as_deref(), Some("south"));
        assert!(!view.flush());
        assert!(view.burst("recovery", Some("south"), 1000));
        assert_eq!(view.state.connected, None);
        assert!(view.receive(
            &ticket,
            Snapshot {
                instance: "synthetic-source",
                revision: 2001,
                phase: "connected",
                connected: Some("south")
            }
        ));
        view.lost();
        assert!(!view.flush());
        assert_eq!(view.state, State::default());
        assert!(!view.available());
        assert!(!view.burst("connected", Some("south"), 1000));
        view.attach();
        assert!(!view.receive(
            &ticket,
            Snapshot {
                instance: "synthetic-source",
                revision: 9000,
                phase: "connected",
                connected: Some("south")
            }
        ));
        assert!(view.burst("connected", Some("north"), 1));
        assert_eq!(view.state.sample, 1);
        assert_eq!(view.state.connected.as_deref(), Some("north"));
        assert!(view.burst("unknown-phase", Some("north"), 1));
        assert_eq!(view.state.phase, "unverified");
        assert_eq!(view.state.connected, None);
    }

    #[test]
    fn all_fixture_scenes_project_after_fresh_attachment() {
        for scene in crate::Fixtures::load().scenes {
            let mut view = UpdateView::default();
            view.attach();
            assert!(view.available());
            assert!(view.burst(&scene.phase, scene.connected.as_deref(), 1));
            assert_eq!(view.state.phase, scene.phase);
            assert_eq!(
                view.state.connected.as_deref(),
                scene.confirmed_id().or_else(|| scene.previous_id())
            );
        }
    }
}
