//! Finite synthetic producer; no daemon, transport, timers or credentials.
use crate::freshness::{Freshness, Snapshot, Ticket};

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
    pending: Option<State>,
    pub state: State,
}

impl UpdateView {
    pub fn attach(&mut self) {
        self.freshness.attach("synthetic-source");
        self.pending = None;
        self.state = State::default();
    }

    pub fn receive(&mut self, ticket: &Ticket, snapshot: Snapshot<'_>) -> bool {
        if !self.freshness.receive(ticket, snapshot.clone()) {
            return false;
        }
        self.pending = Some(State {
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
        });
        true
    }

    pub fn flush(&mut self) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        self.state = pending;
        true
    }

    pub fn lost(&mut self) {
        self.freshness.lost();
        self.pending = None;
        self.state = State::default();
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
            self.receive(
                &ticket,
                Snapshot {
                    instance: "synthetic-source",
                    revision,
                    phase,
                    connected,
                },
            );
        }
        self.flush()
    }

    pub fn available(&self) -> bool {
        self.freshness.request().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_slot_refuses_regression_and_loss_clears_queued_confirmation() {
        let mut view = UpdateView::default();
        view.attach();
        let ticket = view.freshness.request().unwrap();
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
            assert_eq!(view.pending.as_ref().unwrap().sample, revision);
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
