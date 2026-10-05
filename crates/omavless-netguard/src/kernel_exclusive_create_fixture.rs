// SPDX-License-Identifier: MIT
//! One historical developer create/readback, never a production owner.
use super::response_diagnostic::{Isolation, root_directory};
use super::*;
use crate::locked_state::LockedState;
use crate::manager_fixture_identity::{EXCLUSIVE_CREATE_FRAMES as FRAMES, Fixture};
use crate::protocol::{Health, Mode, Protection, Request, Response};
use crate::receipt::NamespaceObservation;
use crate::root_state::RootStateStore;
use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

struct Progress {
    next: usize,
    sealed: bool,
}
impl Progress {
    fn once(&mut self, index: usize, write: impl FnOnce(&[u8]) -> Result<()>) -> Result<()> {
        let usable = !self.sealed;
        self.sealed = true;
        require(usable && index == self.next && index < FRAMES.len())?;
        write(FRAMES[index])?;
        self.next += 1;
        self.sealed = self.next == FRAMES.len();
        Ok(())
    }
}

struct Held {
    isolation: Isolation,
    stage: Option<File>,
    frames: Option<File>,
    state: Option<LockedState>,
    creator: Option<FixtureCreator>,
    progress: Progress,
    deadline: Instant,
}
impl Held {
    fn tick(&self) -> Result<()> {
        require(Instant::now() < self.deadline)
    }
    fn phase(&mut self, index: usize) -> Result<()> {
        let output = self.frames.as_ref().ok_or(REFUSE)?;
        let deadline = self.deadline;
        self.progress.once(index, |bytes| {
            require(Instant::now() < deadline)?;
            let result = rustix::io::write(output, bytes);
            require(Instant::now() < deadline)?;
            require(result.is_ok_and(|count| count == bytes.len()))
        })
    }
    fn run(&mut self) -> Result<()> {
        self.tick()?;
        self.isolation.recheck()?;
        self.tick()?;
        let stage = Path::new(Fixture::ExclusiveCreate.stage());
        self.stage = Some(root_directory(stage)?);
        self.tick()?;
        self.frames = Some(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
                .open(stage.join("create.frames"))
                .map_err(|_| REFUSE)?,
        );
        self.tick()?;
        self.phase(0)?;
        let parent = stage.join("state");
        for path in [&parent, &parent.join("omavless-netguard")] {
            self.tick()?;
            DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|_| REFUSE)?;
            self.tick()?;
        }
        self.state = Some(LockedState::from_root(
            RootStateStore::open_test_parent(root_directory(&parent)?, (0, 0), 1001)
                .map_err(|_| REFUSE)?,
        ));
        self.tick()?;
        self.isolation.recheck()?;
        self.tick()?;
        self.creator = Some(FixtureCreator::open(parent)?);
        self.tick()?;
        let creator = self.creator.as_ref().ok_or(REFUSE)?;
        require(
            creator.session.identity == self.isolation.own_id
                && creator.effects == 0
                && creator.created.is_none(),
        )?;
        self.phase(1)?;
        self.phase(2)?;
        let creator = self.creator.as_mut().ok_or(REFUSE)?;
        // Synthetic epoch only. Actual LockedState persists Pending before the
        // existing creator's original absent lease / exclusive-create witness.
        let namespace = NamespaceObservation::Canonical(creator.epoch);
        let response = self
            .state
            .as_mut()
            .ok_or(REFUSE)?
            .request(
                Request::Arm {
                    generation: 7,
                    mode: Mode::Full,
                },
                namespace,
                creator,
            )
            .map_err(|_| REFUSE)?;
        require(matches!(
            response,
            Response::Status {
                protection: Protection::Armed { generation: 7 },
                health: Health::Verified,
                ..
            }
        ))?;
        require(creator.effects == 1 && creator.created.is_some())?;
        self.tick()?;
        self.phase(3)?;
        self.isolation.recheck()?;
        self.creator
            .as_ref()
            .ok_or(REFUSE)?
            .session
            .check(self.deadline)?;
        self.tick()?;
        self.phase(4)?;
        self.phase(5)?;
        self.tick()?;
        self.frames
            .as_ref()
            .ok_or(REFUSE)?
            .sync_all()
            .map_err(|_| REFUSE)?;
        self.tick()?;
        self.stage
            .as_ref()
            .ok_or(REFUSE)?
            .sync_all()
            .map_err(|_| REFUSE)?;
        self.tick()
    }
}

#[test]
#[ignore = "SOURCE-only one create in fresh admitted manager PrivateNetwork; no selector prepared"]
fn one_create() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_EXCLUSIVE_CREATE_WRITER").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        [
            "--exact",
            Fixture::ExclusiveCreate.writer(),
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]
    );
    let deadline = Instant::now().checked_add(Duration::from_secs(5)).unwrap();
    // No effect-bearing owner yet. Once held, every handled failure parks with
    // no diagnostics, follow-up, destruction, delete or cleanup operation.
    let isolation = Isolation::capture_exclusive_create().expect("K1_CREATE_ADMISSION_REFUSED");
    let held = Box::leak(Box::new(Held {
        isolation,
        stage: None,
        frames: None,
        state: None,
        creator: None,
        progress: Progress {
            next: 0,
            sealed: false,
        },
        deadline,
    }));
    std::panic::set_hook(Box::new(|_| {}));
    if !matches!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| held.run())),
        Ok(Ok(()))
    ) {
        loop {
            std::thread::park();
        }
    }
    // Return makes the evidence historical. No live ownership or teardown claim.
}

#[test]
fn fixed_frame_order_and_every_failed_write_prevent_continuation() {
    for cut in 0..FRAMES.len() {
        let mut progress = Progress {
            next: 0,
            sealed: false,
        };
        let mut seen = Vec::new();
        for index in 0..=cut {
            let result = progress.once(index, |bytes| {
                seen.push(bytes.to_vec());
                if index == cut { Err(REFUSE) } else { Ok(()) }
            });
            assert_eq!(result.is_err(), index == cut);
        }
        assert!(progress.once(cut, |_| panic!("no retry")).is_err());
        assert_eq!(seen.len(), cut + 1);
    }
    let mut progress = Progress {
        next: 0,
        sealed: false,
    };
    for index in 0..FRAMES.len() {
        progress.once(index, |_| Ok(())).unwrap();
    }
    assert!(progress.once(0, |_| panic!("no replay")).is_err());
}
