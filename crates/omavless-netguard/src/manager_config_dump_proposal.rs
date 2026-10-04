//! Pure observed-grammar proposal, NOT an authenticated configuration admission.
//! No production caller, I/O, bus, lifetime token or lifecycle authorization.
use std::collections::BTreeMap;

const UNIT: &str = "omavless-k1-effective-config-reference.service";
const FRAGMENT: &str = "/run/systemd/system/omavless-k1-effective-config-reference.service";
const UNIT_SHA: &str = "01464f072443481b5f45893e39c52ae801b3e60739201f58a307c0dec9e316f1";
const HEADER: &str = "→ Unit omavless-k1-effective-config-reference.service:";
const EXEC: &str = "\t→ ExecStart:";
const COMMAND: &str = "\t\tCommand Line: /usr/bin/false";
const SELECTED: &[(&str, &str)] = &[
    ("Fragment Path", FRAGMENT),
    ("WatchdogSec", "0"),
    ("StandardOutput", "append"),
    ("StandardError", "append"),
    (
        "StandardOutputFileToAppend",
        "/run/omavless-k1-effective-config-reference/native.stdout",
    ),
    (
        "StandardErrorFileToAppend",
        "/run/omavless-k1-effective-config-reference/native.stderr",
    ),
    ("RefuseManualStart", "yes"),
    ("Type", "oneshot"),
    ("User", "root"),
    ("Group", "root"),
];

// Public field LABELS only, independently inspected in the private capture.
// Values are opaque bounded text and confer no fact/authority. This is not a
// generic systemd parser: an unfamiliar field requires a new reviewed grammar.
const OTHER_LABELS: &str = "Description|Instance|Unit Load State|Unit Active State|State Change Timestamp|Inactive Exit Timestamp|Active Enter Timestamp|Active Exit Timestamp|Inactive Enter Timestamp|May GC|Need Daemon Reload|Transient|Perpetual|Garbage Collection Mode|Slice|CGroup|CGroup own mask|Requires|Conflicts|Before|After|References|InSlice|StopWhenUnneeded|RefuseManualStop|DefaultDependencies|SurviveFinalKillSignal|OnSuccessJobMode|OnFailureJobMode|IgnoreOnIsolate|Service State|Result|Reload Result|Clean Result|LiveMount Result|PermissionsStartOnly|RootDirectoryStartOnly|RemainAfterExit|GuessMainPID|Restart|NotifyAccess|NotifyState|OOMPolicy|ReloadSignal|RestartSec|RestartSteps|RestartMaxDelaySec|TimeoutStartSec|TimeoutStopSec|TimeoutStartFailureMode|TimeoutStopFailureMode|RuntimeMaxSec|RuntimeRandomizedExtraSec|KillMode|KillSignal|RestartKillSignal|FinalKillSignal|SendSIGKILL|SendSIGHUP|UMask|WorkingDirectory|RootDirectory|RootEphemeral|NonBlocking|PrivateTmp|PrivateDevices|ProtectKernelTunables|ProtectKernelModules|ProtectKernelLogs|ProtectClock|ProtectControlGroups|PrivateNetwork|PrivateUsers|PrivatePIDs|ProtectHome|ProtectSystem|MountAPIVFS|BindLogSockets|IgnoreSIGPIPE|MemoryDenyWriteExecute|RestrictRealtime|RestrictSUIDSGID|KeyringMode|ProtectHostname|ProtectProc|ProcSubset|MemoryTHP|PrivateBPF|RuntimeDirectoryPreserve|RuntimeDirectoryMode|StateDirectoryMode|CacheDirectoryMode|LogsDirectoryMode|ConfigurationDirectoryMode|TimeoutCleanSec|LimitNOFILE|LimitNOFILESoft|LimitMEMLOCK|LimitMEMLOCKSoft|StandardInput|CapabilityBoundingSet|DynamicUser|LockPersonality|RestrictNamespaces|SystemCallErrorNumber|IOAccounting|MemoryAccounting|TasksAccounting|IPAccounting|CPUWeight|StartupCPUWeight|CPUQuotaPerSecSec|CPUQuotaPeriodSec|AllowedCPUs|StartupAllowedCPUs|AllowedMemoryNodes|StartupAllowedMemoryNodes|CPUSetPartition|IOWeight|StartupIOWeight|MemoryMin|MemoryLow|StartupMemoryLow|MemoryHigh|StartupMemoryHigh|MemoryMax|StartupMemoryMax|MemorySwapMax|StartupMemorySwapMax|MemoryZSwapMax|StartupMemoryZSwapMax|MemoryZSwapWriteback|TasksMax|DevicePolicy|DisableControllers|Delegate|ManagedOOMSwap|ManagedOOMMemoryPressure|ManagedOOMMemoryPressureLimit|ManagedOOMPreference|MemoryPressureWatch|CPUPressureWatch|IOPressureWatch|CoredumpReceive|MemoryPressureThresholdSec|CPUPressureThresholdSec|IOPressureThresholdSec|Bus Ref";

// All arguments are UNAUTHENTICATED pure-test inputs. A version spelling here
// is a hypothesis, not proof of the manager/package version. Never call this
// against old evidence and label its result configuration admission.
fn proposed_text_matches(
    assumed_version: &str,
    observed_unit: &str,
    observed_fragment: &str,
    assumed_unit_sha: &str,
    dump: &str,
) -> Result<(), ()> {
    if assumed_version != "261.2"
        || observed_unit != UNIT
        || observed_fragment != FRAGMENT
        || assumed_unit_sha != UNIT_SHA
        || dump.len() > 64 * 1024
        || !dump.ends_with('\n')
        || dump
            .bytes()
            .any(|b| (b < 32 && b != b'\t' && b != b'\n') || b == 127)
    {
        return Err(());
    }
    let mut lines = dump.split_terminator('\n');
    if lines.next() != Some(HEADER) {
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
            if exec || lines.next() != Some(COMMAND) {
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
        if let Some((_, expected)) = SELECTED.iter().find(|(key, _)| *key == label) {
            if value != *expected || *occurrences != 1 {
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
        || SELECTED
            .iter()
            .any(|(label, _)| seen.get(label) != Some(&1))
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> String {
        let mut text = format!("{HEADER}\n");
        for (label, value) in SELECTED {
            text.push_str(&format!("\t{label}: {value}\n"));
        }
        text.push_str(&format!("{EXEC}\n{COMMAND}\n"));
        text
    }

    fn check(text: &str) -> Result<(), ()> {
        proposed_text_matches("261.2", UNIT, FRAGMENT, UNIT_SHA, text)
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
                proposed_text_matches("261.2", fields[0], fields[1], fields[2], &synthetic())
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
            assert!(check(&synthetic().replace("/usr/bin/false", bad)).is_err());
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
