//! Synthetic G1a client projection experiment. No daemon transport or wire types.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    epoch: u64,
    instance: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot<'a> {
    pub instance: &'a str,
    pub revision: u64,
    pub phase: &'a str,
    pub connected: Option<&'a str>,
}

#[derive(Default)]
pub struct Freshness {
    epoch: u64,
    instance: Option<String>,
    revision: Option<u64>,
    confirmed: Option<String>,
}

impl Freshness {
    /// A simulated successful hello establishes the only instance eligible
    /// to populate this view. Existing connection evidence is discarded.
    pub fn attach(&mut self, instance: &str) {
        self.epoch = self.epoch.wrapping_add(1);
        self.instance = Some(instance.to_owned());
        self.revision = None;
        self.confirmed = None;
    }

    pub fn request(&self) -> Option<Ticket> {
        Some(Ticket {
            epoch: self.epoch,
            instance: self.instance.clone()?,
        })
    }

    /// Only a response from the current simulated instance and a newer
    /// revision can update the view. Unknown phases cannot confirm a tunnel.
    pub fn receive(&mut self, ticket: &Ticket, snapshot: Snapshot<'_>) -> bool {
        if ticket.epoch != self.epoch
            || self.instance.as_deref() != Some(ticket.instance.as_str())
            || snapshot.instance != ticket.instance
            || self
                .revision
                .is_some_and(|revision| snapshot.revision <= revision)
        {
            return false;
        }
        self.revision = Some(snapshot.revision);
        self.confirmed = if snapshot.phase == "connected" {
            snapshot.connected.map(str::to_owned)
        } else {
            None
        };
        true
    }

    pub fn lost(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.instance = None;
        self.revision = None;
        self.confirmed = None;
    }

    pub fn confirmed(&self) -> Option<&str> {
        self.confirmed.as_deref()
    }

    pub fn revision(&self) -> Option<u64> {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_and_regressing_responses_cannot_restore_a_connection() {
        let mut view = Freshness::default();
        view.attach("synthetic-a");
        let first = view.request().unwrap();
        assert!(view.receive(
            &first,
            Snapshot {
                instance: "synthetic-a",
                revision: 8,
                phase: "connected",
                connected: Some("south")
            }
        ));
        assert_eq!(view.confirmed(), Some("south"));

        let later = view.request().unwrap();
        assert!(view.receive(
            &later,
            Snapshot {
                instance: "synthetic-a",
                revision: 9,
                phase: "recovery",
                connected: Some("south")
            }
        ));
        assert_eq!(view.confirmed(), None);
        assert!(!view.receive(
            &first,
            Snapshot {
                instance: "synthetic-a",
                revision: 8,
                phase: "connected",
                connected: Some("south")
            }
        ));
        assert_eq!(view.revision(), Some(9));

        view.lost();
        assert_eq!(view.confirmed(), None);
        assert!(view.request().is_none());
        assert!(!view.receive(
            &later,
            Snapshot {
                instance: "synthetic-a",
                revision: 10,
                phase: "connected",
                connected: Some("south")
            }
        ));

        view.attach("synthetic-b");
        let current = view.request().unwrap();
        assert!(!view.receive(
            &current,
            Snapshot {
                instance: "synthetic-a",
                revision: 11,
                phase: "connected",
                connected: Some("south")
            }
        ));
        assert!(view.receive(
            &current,
            Snapshot {
                instance: "synthetic-b",
                revision: 1,
                phase: "connected",
                connected: Some("north")
            }
        ));
        assert_eq!(view.confirmed(), Some("north"));
        assert!(!view.receive(
            &current,
            Snapshot {
                instance: "synthetic-b",
                revision: 1,
                phase: "connected",
                connected: Some("south")
            }
        ));
        assert_eq!(view.confirmed(), Some("north"));
    }
}
