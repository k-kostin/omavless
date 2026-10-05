//! SOURCE-only one-create manager observer. No Stop/Unref or owner reconstruction.
use super::*;
use crate::manager_fixture_identity::EXCLUSIVE_CREATE_FRAMES;

const TEST: &str = "manager_retained_lifecycle::adapter::exclusive_create::observe_one_create";

struct Held {
    real: Real,
    sealed: bool,
    deadline: Instant,
}
impl Held {
    fn tick(&self) -> Result<()> {
        ensure(Instant::now() < self.deadline)
    }
    fn once(&mut self) -> Result<()> {
        ensure(!self.sealed)?;
        self.sealed = true;
        self.tick()?;
        self.real.admit_retaining_reference()?;
        self.tick()?;
        let original_job = self.real.start_once()?;
        self.tick()?;
        let mut completed = false;
        for _ in 0..MAX_POLLS {
            self.tick()?;
            let observation = self.real.observe_start(&original_job)?;
            self.tick()?;
            if observation == StartObservation::Completed {
                completed = true;
                break;
            }
            self.real.pause_between_known_pending();
            self.tick()?;
        }
        ensure(completed && self.real.completed.is_some())?;
        // Only the original manager's positively completed invocation permits
        // opening this new fixed result. Never inspect output after uncertainty.
        self.real.recheck(true)?;
        self.tick()?;
        let path = Path::new(Fixture::ExclusiveCreate.stage()).join("create.frames");
        self.real.pins.push(Pin::open(path, 0o600, 4096)?);
        self.tick()?;
        ensure(self.real.pins.last().ok_or(())?.bytes()? == EXCLUSIVE_CREATE_FRAMES.concat())?;
        self.tick()?;
        self.real.recheck(true)?;
        self.tick()?;
        let bytes = b"K1_EXCLUSIVE_CREATE_HISTORICAL_KNOWN_ZERO_NOT_CANONICAL\n";
        let written = rustix::io::write(rustix::stdio::stdout(), bytes);
        self.tick()?;
        ensure(written.is_ok_and(|count| count == bytes.len()))?;
        self.tick()
    }
}

fn build() -> Result<Real> {
    let fixture = Fixture::ExclusiveCreate;
    let stage_path = fixture.stage();
    let mut dirs = Vec::new();
    for path in ["/", "/run", "/run/systemd", "/run/systemd/system", stage_path] {
        dirs.push(Directory::open(Path::new(path), path == stage_path)?);
    }
    let unit = Pin::open(Path::new(stage_path).join("fixture.service"), 0o600, 16384)?;
    ensure(unit.bytes()? == fixture.unit_bytes())?;
    let probe = Pin::open(Path::new(stage_path).join("probe"), 0o500, 128 * 1024 * 1024)?;
    let own = File::open("/proc/self/exe").map_err(|_| ())?;
    ensure(meta(&own.metadata().map_err(|_| ())?) == meta(&probe.initial))?;
    let link = std::fs::symlink_metadata(fixture.fragment()).map_err(|_| ())?;
    ensure(link.file_type().is_symlink() && link.uid() == 0 && link.gid() == 0 && link.nlink() == 1)?;
    let stage = dirs.last().ok_or(())?.file.try_clone().map_err(|_| ())?;
    // Fixed disposable-VM administrative bus is trusted here. Unique-owner /
    // version / typed properties do not establish canonical PID1 provenance.
    let connection = zbus::blocking::connection::Builder::address("unix:path=/run/dbus/system_bus_socket")
        .map_err(|_| ())?.max_queued(8).method_timeout(Duration::from_secs(5))
        .build().map_err(|_| ())?;
    Ok(Real {
        fixture, witness: None,
        admitted: admission::Held { connection: admission::FixedConnection { connection, fixture }, stage },
        dirs, pins: vec![unit, probe], link, owner: String::new(), sequence: 0,
        completed: None, seen: None, start: None, stop: None,
        native_verified: false, stopped_verified: false,
    })
}

#[test]
#[ignore = "SOURCE-only fresh one-create manager scope; no publication or actual selector prepared"]
fn observe_one_create() {
    assert_eq!(std::env::var("OMAVLESS_K1_EXCLUSIVE_CREATE_MANAGER").as_deref(), Ok("1"));
    assert_eq!(std::env::args().skip(1).collect::<Vec<_>>(), [
        "--exact", TEST, "--ignored", "--nocapture", "--test-threads=1",
    ]);
    assert_eq!(nix::unistd::getuid().as_raw(), 0);
    assert_eq!(nix::unistd::geteuid().as_raw(), 0);
    let deadline = Instant::now().checked_add(PHASE_BUDGET).unwrap();
    // Build has no Ref/Start effect; returned owners retained before admission.
    // Partial constructors and fatal death are not solved by Box::leak.
    let held = Box::leak(Box::new(Held { real: build().expect("K1_CREATE_MANAGER_PRE_EFFECT_REFUSED"), sealed: false, deadline }));
    std::panic::set_hook(Box::new(|_| {}));
    if !matches!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| held.once())), Ok(Ok(()))) {
        loop { std::thread::park(); }
    }
    // No StopUnit/UnrefUnit, output fetch, cgroup query or cleanup follows.
    // On return, this is historical evidence only; no live reference is promised.
}

#[test]
fn frame_grammar_never_accepts_prefix_or_duplicate_completion() {
    let wanted = EXCLUSIVE_CREATE_FRAMES.concat();
    assert_eq!(wanted.iter().filter(|byte| **byte == b'\n').count(), 6);
    for end in 0..wanted.len() { assert_ne!(&wanted[..end], wanted.as_slice()); }
    let mut extra = wanted.clone();
    extra.extend_from_slice(EXCLUSIVE_CREATE_FRAMES[5]);
    assert_ne!(extra, wanted);
    let mut swapped = EXCLUSIVE_CREATE_FRAMES;
    swapped.swap(2, 3);
    assert_ne!(swapped.concat(), wanted);
}
