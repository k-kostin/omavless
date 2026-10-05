//! Drives the real constructor/check sequence, replacing only fixed leaf calls.
//! Uses ordinary /dev/null descriptors and proc/self/fd metadata; no namespace
//! ioctl, netlink socket, datagram, service, subprocess or VM operation.
use super::*;

#[derive(Clone, Copy)]
enum Cut {
    Error,
    Panic,
    Late,
}
struct Fake {
    now: Instant,
    calls: usize,
    cut: Option<(usize, Cut)>,
    opened: Vec<i32>,
    created: Option<i32>,
    cookies: Vec<i32>,
    next_cookie: u64,
    step_elapsed: Duration,
}
impl Fake {
    fn new() -> Self {
        Self {
            now: Instant::now(),
            calls: 0,
            cut: None,
            opened: Vec::new(),
            created: None,
            cookies: Vec::new(),
            next_cookie: 7,
            step_elapsed: Duration::ZERO,
        }
    }
    fn step(&mut self) -> Result<(), Refused> {
        self.calls += 1;
        self.now = self.now.checked_add(self.step_elapsed).unwrap();
        if let Some((at, kind)) = self.cut {
            if at == self.calls {
                match kind {
                    Cut::Error => return Err(Refused::Unavailable),
                    Cut::Panic => panic!("inert leaf panic"),
                    Cut::Late => self.now = self.now.checked_add(Duration::from_secs(3)).unwrap(),
                }
            }
        }
        Ok(())
    }
    fn file(&mut self) -> File {
        let file = File::open("/dev/null").unwrap();
        self.opened.push(file.as_raw_fd());
        file
    }
}
impl Queries for Fake {
    fn now(&mut self) -> Instant {
        self.now
    }
    fn open_current(&mut self) -> Result<File, Refused> {
        self.step()?;
        Ok(self.file())
    }
    fn kind(&mut self, _: &File) -> Result<NamespaceType, Refused> {
        self.step()?;
        Ok(NamespaceType::Network)
    }
    fn id(&mut self, _: &File) -> Result<u64, Refused> {
        self.step()?;
        Ok(7)
    }
    fn create(&mut self) -> Result<OwnedFd, Refused> {
        self.step()?;
        let file = self.file();
        assert!(self.created.replace(file.as_raw_fd()).is_none());
        Ok(file.into())
    }
    fn bind(&mut self, fd: &OwnedFd) -> Result<(), Refused> {
        assert_eq!(Some(fd.as_raw_fd()), self.created);
        self.step()
    }
    fn cookie(&mut self, fd: &OwnedFd) -> Result<u64, Refused> {
        assert_eq!(Some(fd.as_raw_fd()), self.created);
        self.cookies.push(fd.as_raw_fd());
        self.step()?;
        Ok(self.next_cookie)
    }
}
fn retained(fd: i32) {
    // Intentionally inspect only this process's synthetic ordinary descriptors.
    assert!(std::fs::metadata(format!("/proc/self/fd/{fd}")).is_ok());
}

#[test]
fn original_attempt_budget_spent_at_handoff_is_not_reset_by_constructor() {
    let mut fake = Fake::new();
    let attempt = FixedAttempt {
        deadline: fake.now + Duration::from_secs(2),
        thread: thread::current().id(),
        attempted: Cell::new(false),
        _same_thread: PhantomData,
    };
    let anchor = File::open("/dev/null").unwrap();
    let fd = anchor.as_raw_fd();
    let result = attempt.run_with(&mut fake, |q| {
        q.now += Duration::from_millis(1900);
        q.step_elapsed = Duration::from_millis(110);
        Ok(anchor)
    });
    assert_eq!(result, Err(Refused::Expired));
    assert_eq!(fake.calls, 1); // Late current opener; NO subsequent namespace-type ioctl.
    retained(fd);
    retained(fake.opened[0]);
    assert_eq!(
        attempt.run_with(&mut fake, |_| panic!("must not reopen")),
        Err(Refused::Sealed)
    );
    assert_eq!(fake.calls, 1);
}

#[test]
fn fixed_attempt_late_open_error_and_panic_never_reopen() {
    for cut in [Cut::Late, Cut::Error, Cut::Panic] {
        let mut fake = Fake::new();
        let attempt = FixedAttempt {
            deadline: fake.now + Duration::from_secs(2),
            thread: thread::current().id(),
            attempted: Cell::new(false),
            _same_thread: PhantomData,
        };
        let mut fd = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            attempt.run_with(&mut fake, |q| match cut {
                Cut::Error => Err(Refused::Unavailable),
                Cut::Panic => panic!("synthetic opener panic"),
                Cut::Late => {
                    let file = q.file();
                    fd = Some(file.as_raw_fd());
                    q.now += Duration::from_secs(3);
                    Ok(file)
                }
            })
        }));
        match cut {
            Cut::Panic => assert!(result.is_err()),
            Cut::Late => assert_eq!(result.unwrap(), Err(Refused::Expired)),
            Cut::Error => assert_eq!(result.unwrap(), Err(Refused::Unavailable)),
        }
        assert_eq!(fake.calls, 0);
        if let Some(fd) = fd {
            retained(fd);
        }
        assert_eq!(
            attempt.run_with(&mut fake, |_| panic!("must not reopen")),
            Err(Refused::Sealed)
        );
    }
}

#[test]
fn fixed_attempt_success_uses_same_creator_and_cannot_be_repeated() {
    let mut fake = Fake::new();
    let attempt = FixedAttempt {
        deadline: fake.now + Duration::from_secs(2),
        thread: thread::current().id(),
        attempted: Cell::new(false),
        _same_thread: PhantomData,
    };
    assert_eq!(attempt.run_with(&mut fake, |q| Ok(q.file())), Ok(()));
    assert_eq!(fake.calls, 23);
    assert_eq!(fake.cookies, vec![fake.created.unwrap(); 2]);
    retained(fake.created.unwrap());
    assert_eq!(
        attempt.run_with(&mut fake, |_| panic!("must not reopen")),
        Err(Refused::Sealed)
    );
    assert_eq!(fake.calls, 23);
}

#[test]
fn every_constructor_leaf_error_panic_or_late_return_stops_and_retains_originals() {
    for cut in [Cut::Error, Cut::Panic, Cut::Late] {
        for at in 1..=15 {
            let mut fake = Fake::new();
            fake.cut = Some((at, cut));
            let anchor = File::open("/dev/null").unwrap();
            let anchor_fd = anchor.as_raw_fd();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                LocalBinding::bind_with(anchor, &mut fake)
            }));
            match cut {
                Cut::Panic => assert!(result.is_err()),
                Cut::Error => assert!(matches!(result.unwrap(), Err(Refused::Unavailable))),
                Cut::Late => assert!(matches!(result.unwrap(), Err(Refused::Expired))),
            }
            assert_eq!(fake.calls, at);
            retained(anchor_fd);
            for fd in fake.opened {
                retained(fd);
            }
        }
    }
}

#[test]
fn every_verify_leaf_cut_permanently_refuses_next_call_with_same_creator_retained() {
    for cut in [Cut::Error, Cut::Panic, Cut::Late] {
        for at in 16..=23 {
            let mut fake = Fake::new();
            let anchor = File::open("/dev/null").unwrap();
            let mut owner = LocalBinding::bind_with(anchor, &mut fake).unwrap();
            assert_eq!(fake.calls, 15);
            fake.opened.clear(); // Successful constructor already released its temporary read FD.
            fake.cut = Some((at, cut));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.verify_queries(&mut fake)
            }));
            match cut {
                Cut::Panic => assert!(result.is_err()),
                Cut::Error => assert_eq!(result.unwrap(), Err(Refused::Unavailable)),
                Cut::Late => assert_eq!(result.unwrap(), Err(Refused::Expired)),
            }
            assert_eq!(fake.calls, at);
            assert_eq!(owner.verify_queries(&mut fake), Err(Refused::Sealed));
            assert_eq!(fake.calls, at);
            retained(owner.originals.anchor.as_raw_fd());
            retained(
                owner
                    .originals
                    .thread_namespace
                    .as_ref()
                    .unwrap()
                    .as_raw_fd(),
            );
            retained(owner.originals.creator.as_ref().unwrap().as_raw_fd());
            for fd in fake.opened {
                retained(fd);
            }
        }
    }
}

#[test]
fn success_rechecks_same_original_creator_and_ordinary_drop_retains_it() {
    let (anchor, ns, creator) = {
        let mut fake = Fake::new();
        let mut owner =
            LocalBinding::bind_with(File::open("/dev/null").unwrap(), &mut fake).unwrap();
        owner.verify_queries(&mut fake).unwrap();
        assert_eq!(fake.calls, 23);
        assert_eq!(fake.cookies, vec![fake.created.unwrap(); 2]);
        (
            owner.originals.anchor.as_raw_fd(),
            owner
                .originals
                .thread_namespace
                .as_ref()
                .unwrap()
                .as_raw_fd(),
            owner.originals.creator.as_ref().unwrap().as_raw_fd(),
        )
    };
    for fd in [anchor, ns, creator] {
        retained(fd);
    }
}

#[test]
fn cookie_mismatch_never_reopens_or_creates_a_replacement_socket() {
    let mut fake = Fake::new();
    let mut owner = LocalBinding::bind_with(File::open("/dev/null").unwrap(), &mut fake).unwrap();
    fake.next_cookie = 8;
    assert_eq!(owner.verify_queries(&mut fake), Err(Refused::Mismatch));
    let calls = fake.calls;
    fake.next_cookie = 7;
    assert_eq!(owner.verify_queries(&mut fake), Err(Refused::Sealed));
    assert_eq!(fake.calls, calls);
    retained(fake.created.unwrap());
}
