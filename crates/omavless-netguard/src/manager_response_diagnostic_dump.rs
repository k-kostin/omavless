//! Pure fixed-version configured facts, never lifecycle/ownership admission.
//! Caller must bind same-owner typed Version and original unit FD separately.
use crate::manager_fixture_identity::Fixture;
use std::collections::BTreeMap;

const UNIT: &str = "omavless-k1-retained-private-lifecycle.service";
const FRAGMENT: &str = "/run/systemd/system/omavless-k1-retained-private-lifecycle.service";
pub(super) const UNIT_SHA: &str =
    "198730a79751ccee045c6173d4cbb75db7ece33a784f5390f255cb65aa5e72b5";
const HEADER: &str = "→ Unit omavless-k1-retained-private-lifecycle.service:";
const EXEC: &str = "\t→ ExecStart:";
const COMMAND: &str = "\t\tCommand Line: /run/omavless-k1-retained-private-lifecycle/probe --exact kernel_observer::creator_lifecycle::response_diagnostic::manager_private_lifecycle --ignored --nocapture --test-threads=1";
const SELECTED: &[(&str, &str)] = &[
    ("Fragment Path", FRAGMENT),
    ("WatchdogSec", "0"),
    ("StandardOutput", "append"),
    ("StandardError", "append"),
    (
        "StandardOutputFileToAppend",
        "/run/omavless-k1-retained-private-lifecycle/native.stdout",
    ),
    (
        "StandardErrorFileToAppend",
        "/run/omavless-k1-retained-private-lifecycle/native.stderr",
    ),
    ("RefuseManualStart", "no"),
    ("Type", "oneshot"),
    ("User", "root"),
    ("Group", "root"),
    ("Environment", "OMAVLESS_K1_RETAINED_LIFECYCLE_WRITER=1"),
    ("Open File", "/proc/1/ns/net:k1-host-netns:read-only"),
];

// Public field LABELS only, independently inspected in the private capture.
// Values are opaque bounded text and confer no fact/authority. This is not a
// generic systemd parser: an unfamiliar field requires a new reviewed grammar.
const OTHER_LABELS: &str = "Description|Instance|Unit Load State|Unit Active State|State Change Timestamp|Inactive Exit Timestamp|Active Enter Timestamp|Active Exit Timestamp|Inactive Enter Timestamp|May GC|Need Daemon Reload|Transient|Perpetual|Garbage Collection Mode|Slice|CGroup|CGroup own mask|Requires|Conflicts|Before|After|References|InSlice|StopWhenUnneeded|RefuseManualStop|DefaultDependencies|SurviveFinalKillSignal|OnSuccessJobMode|OnFailureJobMode|IgnoreOnIsolate|Service State|Result|Reload Result|Clean Result|LiveMount Result|PermissionsStartOnly|RootDirectoryStartOnly|RemainAfterExit|GuessMainPID|Restart|NotifyAccess|NotifyState|OOMPolicy|ReloadSignal|RestartSec|RestartSteps|RestartMaxDelaySec|TimeoutStartSec|TimeoutStopSec|TimeoutStartFailureMode|TimeoutStopFailureMode|RuntimeMaxSec|RuntimeRandomizedExtraSec|KillMode|KillSignal|RestartKillSignal|FinalKillSignal|SendSIGKILL|SendSIGHUP|UMask|WorkingDirectory|RootDirectory|RootEphemeral|NonBlocking|PrivateTmp|PrivateDevices|ProtectKernelTunables|ProtectKernelModules|ProtectKernelLogs|ProtectClock|ProtectControlGroups|PrivateNetwork|PrivateUsers|PrivatePIDs|ProtectHome|ProtectSystem|MountAPIVFS|BindLogSockets|IgnoreSIGPIPE|MemoryDenyWriteExecute|RestrictRealtime|RestrictSUIDSGID|KeyringMode|ProtectHostname|ProtectProc|ProcSubset|MemoryTHP|PrivateBPF|RuntimeDirectoryPreserve|RuntimeDirectoryMode|StateDirectoryMode|CacheDirectoryMode|LogsDirectoryMode|ConfigurationDirectoryMode|TimeoutCleanSec|LimitNOFILE|LimitNOFILESoft|LimitMEMLOCK|LimitMEMLOCKSoft|StandardInput|CapabilityBoundingSet|DynamicUser|LockPersonality|RestrictNamespaces|SystemCallErrorNumber|IOAccounting|MemoryAccounting|TasksAccounting|IPAccounting|CPUWeight|StartupCPUWeight|CPUQuotaPerSecSec|CPUQuotaPeriodSec|AllowedCPUs|StartupAllowedCPUs|AllowedMemoryNodes|StartupAllowedMemoryNodes|CPUSetPartition|IOWeight|StartupIOWeight|MemoryMin|MemoryLow|StartupMemoryLow|MemoryHigh|StartupMemoryHigh|MemoryMax|StartupMemoryMax|MemorySwapMax|StartupMemorySwapMax|MemoryZSwapMax|StartupMemoryZSwapMax|MemoryZSwapWriteback|TasksMax|DevicePolicy|DisableControllers|Delegate|ManagedOOMSwap|ManagedOOMMemoryPressure|ManagedOOMMemoryPressureLimit|ManagedOOMPreference|MemoryPressureWatch|CPUPressureWatch|IOPressureWatch|CoredumpReceive|MemoryPressureThresholdSec|CPUPressureThresholdSec|IOPressureThresholdSec|Bus Ref";

// Pure inputs alone are not authority. Only the new fixed capture caller binds
// the exact same-connection manager version; the outer retains original unit FD.
pub(super) fn proposed_text_matches(
    assumed_version: &str,
    observed_unit: &str,
    observed_fragment: &str,
    assumed_unit_sha: &str,
    dump: &str,
) -> Result<(), ()> {
    proposed_text_matches_for(
        Fixture::PrivateLifecycle,
        assumed_version,
        observed_unit,
        observed_fragment,
        assumed_unit_sha,
        dump,
    )
}

pub(super) fn proposed_text_matches_for(
    fixture: Fixture,
    assumed_version: &str,
    observed_unit: &str,
    observed_fragment: &str,
    assumed_unit_sha: &str,
    dump: &str,
) -> Result<(), ()> {
    if assumed_version != "261.2-1-arch"
        || observed_unit != fixture.unit()
        || observed_fragment != fixture.fragment()
        || assumed_unit_sha != fixture.unit_sha()
        || dump.len() > 64 * 1024
        || !dump.ends_with('\n')
        || dump
            .bytes()
            .any(|b| (b < 32 && b != b'\t' && b != b'\n') || b == 127)
    {
        return Err(());
    }
    let mut lines = dump.split_terminator('\n');
    let header = format!("→ Unit {}:", fixture.unit());
    let command = format!(
        "\t\tCommand Line: {}/probe --exact {} --ignored --nocapture --test-threads=1",
        fixture.stage(),
        fixture.writer()
    );
    let selected = selected_for(fixture);
    if lines.next() != Some(header.as_str()) {
        return Err(());
    }
    let mut seen = BTreeMap::new();
    let mut exec = false;
    let mut count = 1;
    while let Some(line) = lines.next() {
        count += 1;
        if count > 256 || line.len() > 4096 {
            return Err(());
        }
        if line == EXEC {
            if exec || lines.next() != Some(command.as_str()) {
                return Err(());
            }
            exec = true;
            count += 1;
            if count > 256 {
                return Err(());
            }
            continue;
        }
        let field = line.strip_prefix('\t').ok_or(())?;
        let (label, value) = field.split_once(": ").ok_or(())?;
        // One exact tab only. No normalization, trimming, alternate Unicode,
        // control bytes or extra command/subsection structures are permitted.
        if !field.bytes().all(|b| (32..=126).contains(&b)) {
            return Err(());
        }
        let occurrences = seen.entry(label).or_insert(0_usize);
        *occurrences += 1;
        if let Some((_, expected)) = selected.iter().find(|(key, _)| *key == label) {
            if value != expected || *occurrences != 1 {
                return Err(());
            }
        } else if !OTHER_LABELS.split('|').any(|known| label == known)
            || (!matches!(
                label,
                "Requires" | "Conflicts" | "Before" | "After" | "References" | "InSlice"
            ) && *occurrences != 1)
            || *occurrences > 16
        {
            return Err(());
        }
    }
    if !exec
        || selected
            .iter()
            .any(|(label, _)| seen.get(label) != Some(&1))
    {
        return Err(());
    }
    Ok(())
}

fn selected_for(fixture: Fixture) -> Vec<(&'static str, String)> {
    SELECTED
        .iter()
        .map(|(label, original)| {
            let value = match *label {
                "Fragment Path" => fixture.fragment().to_owned(),
                "StandardOutputFileToAppend" => format!("{}/native.stdout", fixture.stage()),
                "StandardErrorFileToAppend" => format!("{}/native.stderr", fixture.stage()),
                "Environment" => fixture.environment().to_owned(),
                _ => (*original).to_owned(),
            };
            (*label, value)
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(crate) fn synthetic() -> String {
        let mut text = format!("{HEADER}\n");
        for (label, value) in SELECTED {
            text.push_str(&format!("\t{label}: {value}\n"));
        }
        text.push_str(&format!("{EXEC}\n{COMMAND}\n"));
        text
    }

    pub(crate) fn synthetic_for(fixture: Fixture) -> String {
        let mut text = format!("→ Unit {}:\n", fixture.unit());
        for (label, value) in selected_for(fixture) {
            text.push_str(&format!("\t{label}: {value}\n"));
        }
        text.push_str(&format!("{EXEC}\n\t\tCommand Line: {}/probe --exact {} --ignored --nocapture --test-threads=1\n", fixture.stage(), fixture.writer()));
        text
    }

    fn check(text: &str) -> Result<(), ()> {
        proposed_text_matches("261.2-1-arch", UNIT, FRAGMENT, UNIT_SHA, text)
    }

    #[test]
    fn both_fixed_dumps_reject_crossed_identity_command_and_paths() {
        assert_eq!(Fixture::PrivateLifecycle.unit_sha(), UNIT_SHA);
        for fixture in [Fixture::PrivateLifecycle, Fixture::RetainedLease] {
            let mut text = format!("→ Unit {}:\n", fixture.unit());
            for (label, value) in selected_for(fixture) {
                text.push_str(&format!("\t{label}: {value}\n"));
            }
            text.push_str(&format!("{EXEC}\n\t\tCommand Line: {}/probe --exact {} --ignored --nocapture --test-threads=1\n", fixture.stage(), fixture.writer()));
            let validate = |raw: &str| {
                proposed_text_matches_for(
                    fixture,
                    "261.2-1-arch",
                    fixture.unit(),
                    fixture.fragment(),
                    fixture.unit_sha(),
                    raw,
                )
            };
            assert_eq!(validate(&text), Ok(()));
            let other = if fixture == Fixture::PrivateLifecycle {
                Fixture::RetainedLease
            } else {
                Fixture::PrivateLifecycle
            };
            assert!(
                proposed_text_matches_for(
                    other,
                    "261.2-1-arch",
                    fixture.unit(),
                    fixture.fragment(),
                    fixture.unit_sha(),
                    &text
                )
                .is_err()
            );
            for (from, to) in [
                (fixture.unit(), other.unit()),
                (fixture.stage(), other.stage()),
                (fixture.writer(), other.writer()),
                (fixture.environment(), other.environment()),
            ] {
                assert!(validate(&text.replace(from, to)).is_err());
            }
            if fixture == Fixture::PrivateLifecycle {
                assert_eq!(text, synthetic());
            }
        }
    }

    #[test]
    fn synthetic_selected_facts_only_not_live_admission() {
        assert_eq!(check(&synthetic()), Ok(()));
        let extra = format!(
            "{}\tDescription: SYNTHETIC ONLY\n\tRequires: synthetic-a.target\n\tRequires: synthetic-b.target\n",
            synthetic()
        );
        assert_eq!(check(&extra), Ok(()));
    }

    #[test]
    fn version_unit_fragment_and_raw_unit_pin_are_exact_not_prefixes() {
        for version in ["", "261", "261.2-extra", "261.20", "262", " 261.2"] {
            assert!(
                proposed_text_matches(version, UNIT, FRAGMENT, UNIT_SHA, &synthetic()).is_err()
            );
        }
        for which in 0..3 {
            let mut fields = [UNIT, FRAGMENT, UNIT_SHA];
            fields[which] = "unrelated";
            assert!(
                proposed_text_matches(
                    "261.2-1-arch",
                    fields[0],
                    fields[1],
                    fields[2],
                    &synthetic()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn each_required_field_missing_duplicate_or_wrong_refuses() {
        for (label, value) in SELECTED {
            let line = format!("\t{label}: {value}\n");
            for changed in [
                synthetic().replace(&line, ""),
                synthetic().replace(&line, &format!("{line}{line}")),
                synthetic().replace(&line, &format!("\t{label}: wrong\n")),
                synthetic().replace(&line, &format!("\t{label}: {value}suffix\n")),
            ] {
                assert!(check(&changed).is_err());
            }
        }
    }

    #[test]
    fn extra_units_commands_unknowns_and_path_aliases_refuse() {
        for suffix in [
            HEADER,
            "→ Unit other.service:",
            "\t→ ExecStop:",
            "\t\tCommand Line: /usr/bin/true",
            "\tUnknownProperty: no",
            "\tStandardOutputFile: /tmp/other",
            "\tStandardOutputFileToTruncate: /tmp/other",
            "\tStandard Output: append",
            "\tWatchdogSecExtra: 0",
        ] {
            assert!(check(&format!("{}{suffix}\n", synthetic())).is_err());
        }
        for bad in [
            "/usr/bin/false extra",
            "-/usr/bin/false",
            "/bin/false",
            "/usr/bin/true",
        ] {
            assert!(
                check(&synthetic().replace(COMMAND, &format!("\t\tCommand Line: {bad}"))).is_err()
            );
        }
        assert!(check(&synthetic().replace(EXEC, "\t→ ExecStartPre:")).is_err());
        assert!(check(&synthetic().replace(COMMAND, "")).is_err());
        assert!(check(&format!("{}{EXEC}\n{COMMAND}\n", synthetic())).is_err());
    }

    #[test]
    fn indentation_controls_unicode_header_and_bounds_refuse() {
        for changed in [
            synthetic().replace("\tWatchdogSec", " WatchdogSec"),
            synthetic().replace("\tWatchdogSec", "\t\tWatchdogSec"),
            synthetic().replace("WatchdogSec", "WatchdоgSec"), // Cyrillic о
            synthetic().replace("WatchdogSec: 0", "WatchdogSec: 0\r"),
            synthetic().replace("WatchdogSec: 0", "WatchdogSec: 0\0"),
            synthetic().replace("WatchdogSec: 0", "WatchdogSec: \t0"),
            synthetic().replace("WatchdogSec: 0", "WatchdogSec: 0\u{1b}"),
            synthetic().replace(HEADER, "→ Unit other.service:"),
            synthetic().trim_end_matches('\n').to_owned(),
            format!("{}\n", synthetic()),
            format!("{}\tDescription: {}\n", synthetic(), "x".repeat(4096)),
            "x".repeat(64 * 1024 + 1),
        ] {
            assert!(check(&changed).is_err());
        }
        assert!(
            check(&format!(
                "{}{}",
                synthetic(),
                "\tRequires: synthetic.target\n".repeat(17)
            ))
            .is_err()
        );
    }
}
