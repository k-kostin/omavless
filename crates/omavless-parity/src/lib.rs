// SPDX-License-Identifier: MIT

//! Bounded, credential-safe comparison of sanitized migration results.
//!
//! This crate compares reports produced by implementation-specific adapters.
//! It never executes those adapters and never prints their semantic facts or
//! fingerprints. A mismatch exposes only checked public case IDs.

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const API: &str = "omavless.parity";
pub const VERSION: u32 = 1;
pub const MAX_REPORT_BYTES: usize = 1024 * 1024;
pub const MAX_CASES: usize = 4096;
pub const MAX_FACTS_PER_CASE: usize = 64;
pub const MAX_MISMATCH_IDS: usize = 64;
const MAX_SUITE_BYTES: usize = 64;
const MAX_CASE_ID_BYTES: usize = 96;
const MAX_CLASSIFICATION_BYTES: usize = 64;
const MAX_FACT_KEY_BYTES: usize = 64;
const MAX_FACT_TEXT_BYTES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub api: String,
    pub version: u32,
    pub suite: String,
    pub cases: Vec<CaseResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseResult {
    pub id: String,
    pub classification: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub facts: PublicFacts,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct PublicFacts(pub BTreeMap<String, PublicValue>);

impl<'de> Deserialize<'de> for PublicFacts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FactsVisitor;

        impl<'de> Visitor<'de> for FactsVisitor {
            type Value = PublicFacts;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a public parity facts object")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<String, PublicValue>()? {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate public fact"));
                    }
                    if values.len() > MAX_FACTS_PER_CASE {
                        return Err(de::Error::custom("too many public facts"));
                    }
                }
                Ok(PublicFacts(values))
            }
        }

        deserializer.deserialize_map(FactsVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PublicValue {
    Bool(bool),
    Integer(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonSummary {
    pub api: &'static str,
    pub version: u32,
    pub suite: String,
    pub matched: bool,
    pub case_count: usize,
    pub mismatch_count: usize,
    pub mismatch_ids: Vec<String>,
    pub mismatch_ids_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidEncoding,
    InvalidJson,
    InvalidEnvelope,
    InvalidIdentifier,
    InvalidClassification,
    InvalidFingerprint,
    InvalidFacts,
    TooManyCases,
    DuplicateCaseId,
    IncompatibleReports,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidEncoding => "parity report encoding is invalid",
            Self::InvalidJson => "parity report JSON is invalid",
            Self::InvalidEnvelope => "parity report envelope is invalid",
            Self::InvalidIdentifier => "parity report identifier is invalid",
            Self::InvalidClassification => "parity classification is invalid",
            Self::InvalidFingerprint => "parity fingerprint is invalid",
            Self::InvalidFacts => "parity public facts are invalid",
            Self::TooManyCases => "parity report has too many cases",
            Self::DuplicateCaseId => "parity report has duplicate case IDs",
            Self::IncompatibleReports => "parity reports are incompatible",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ErrorCode {}

pub fn parse_report(input: &[u8]) -> Result<Report, ErrorCode> {
    if input.len() > MAX_REPORT_BYTES {
        return Err(ErrorCode::InvalidEnvelope);
    }
    std::str::from_utf8(input).map_err(|_| ErrorCode::InvalidEncoding)?;
    let report: Report = serde_json::from_slice(input).map_err(|_| ErrorCode::InvalidJson)?;
    validate_report(&report)?;
    Ok(report)
}

pub fn validate_report(report: &Report) -> Result<(), ErrorCode> {
    if report.api != API || report.version != VERSION {
        return Err(ErrorCode::InvalidEnvelope);
    }
    if !safe_slug(&report.suite, MAX_SUITE_BYTES) {
        return Err(ErrorCode::InvalidIdentifier);
    }
    if report.cases.len() > MAX_CASES {
        return Err(ErrorCode::TooManyCases);
    }

    let mut ids = BTreeSet::new();
    for case in &report.cases {
        if !safe_slug(&case.id, MAX_CASE_ID_BYTES) {
            return Err(ErrorCode::InvalidIdentifier);
        }
        if !ids.insert(case.id.as_str()) {
            return Err(ErrorCode::DuplicateCaseId);
        }
        if !safe_slug(&case.classification, MAX_CLASSIFICATION_BYTES) {
            return Err(ErrorCode::InvalidClassification);
        }
        if let Some(fingerprint) = &case.fingerprint
            && (fingerprint.len() != 64
                || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit())
                || fingerprint.bytes().any(|byte| byte.is_ascii_uppercase()))
        {
            return Err(ErrorCode::InvalidFingerprint);
        }
        validate_facts(&case.facts)?;
    }
    Ok(())
}

fn validate_facts(facts: &PublicFacts) -> Result<(), ErrorCode> {
    if facts.0.len() > MAX_FACTS_PER_CASE {
        return Err(ErrorCode::InvalidFacts);
    }
    for (key, value) in &facts.0 {
        if !safe_slug(key, MAX_FACT_KEY_BYTES) {
            return Err(ErrorCode::InvalidFacts);
        }
        if let PublicValue::Text(text) = value
            && !safe_slug(text, MAX_FACT_TEXT_BYTES)
        {
            return Err(ErrorCode::InvalidFacts);
        }
    }
    Ok(())
}

fn safe_slug(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn compare_reports(
    reference: &Report,
    candidate: &Report,
) -> Result<ComparisonSummary, ErrorCode> {
    validate_report(reference)?;
    validate_report(candidate)?;
    if reference.api != candidate.api
        || reference.version != candidate.version
        || reference.suite != candidate.suite
    {
        return Err(ErrorCode::IncompatibleReports);
    }

    let reference_cases: BTreeMap<&str, &CaseResult> = reference
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect();
    let candidate_cases: BTreeMap<&str, &CaseResult> = candidate
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect();
    let all_ids: BTreeSet<&str> = reference_cases
        .keys()
        .chain(candidate_cases.keys())
        .copied()
        .collect();
    let case_count = all_ids.len();

    let mut mismatch_count = 0;
    let mut mismatch_ids = Vec::new();
    for id in all_ids {
        if reference_cases.get(id) != candidate_cases.get(id) {
            mismatch_count += 1;
            if mismatch_ids.len() < MAX_MISMATCH_IDS {
                mismatch_ids.push(id.to_owned());
            }
        }
    }

    Ok(ComparisonSummary {
        api: "omavless.parity.result",
        version: VERSION,
        suite: reference.suite.clone(),
        matched: mismatch_count == 0,
        case_count,
        mismatch_count,
        mismatch_ids_truncated: mismatch_count > mismatch_ids.len(),
        mismatch_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &[u8] = br#"{
      "api":"omavless.parity",
      "version":1,
      "suite":"r0-smoke",
      "cases":[{
        "id":"accepted-case",
        "classification":"accepted",
        "fingerprint":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "facts":{"bounded":true,"count":2,"feature":"workspace"}
      }]
    }"#;

    #[test]
    fn valid_report_round_trips() {
        let report = parse_report(VALID).expect("valid report");
        assert_eq!(report.suite, "r0-smoke");
        assert_eq!(report.cases.len(), 1);
    }

    #[test]
    fn duplicate_fields_and_unknown_fields_fail_closed() {
        let duplicate = br#"{"api":"omavless.parity","api":"omavless.parity","version":1,"suite":"x","cases":[]}"#;
        let unknown =
            br#"{"api":"omavless.parity","version":1,"suite":"x","cases":[],"private":"secret"}"#;
        assert_eq!(parse_report(duplicate), Err(ErrorCode::InvalidJson));
        assert_eq!(parse_report(unknown), Err(ErrorCode::InvalidJson));
    }

    #[test]
    fn duplicate_public_fact_fails_closed() {
        let input = br#"{"api":"omavless.parity","version":1,"suite":"x","cases":[{"id":"a","classification":"ok","facts":{"ready":true,"ready":false}}]}"#;
        assert_eq!(parse_report(input), Err(ErrorCode::InvalidJson));
    }

    #[test]
    fn unsafe_identifiers_text_and_fingerprints_are_rejected() {
        let mut report = parse_report(VALID).expect("valid report");
        report.cases[0].id = "private value".to_owned();
        assert_eq!(validate_report(&report), Err(ErrorCode::InvalidIdentifier));

        let mut report = parse_report(VALID).expect("valid report");
        report.cases[0].facts.0.insert(
            "value".to_owned(),
            PublicValue::Text("https://private".to_owned()),
        );
        assert_eq!(validate_report(&report), Err(ErrorCode::InvalidFacts));

        let mut report = parse_report(VALID).expect("valid report");
        report.cases[0].fingerprint = Some("A".repeat(64));
        assert_eq!(validate_report(&report), Err(ErrorCode::InvalidFingerprint));
    }

    #[test]
    fn comparison_is_order_independent_and_exposes_only_case_ids() {
        let reference = parse_report(VALID).expect("valid report");
        let mut candidate = reference.clone();
        candidate.cases[0].classification = "rejected".to_owned();
        candidate.cases[0].facts.0.insert(
            "private".to_owned(),
            PublicValue::Text("never-printed".to_owned()),
        );

        let summary = compare_reports(&reference, &candidate).expect("compatible reports");
        assert!(!summary.matched);
        assert_eq!(summary.mismatch_count, 1);
        assert_eq!(summary.mismatch_ids, vec!["accepted-case"]);
        let public = serde_json::to_string(&summary).expect("serializable summary");
        assert!(!public.contains("never-printed"));
        assert!(!public.contains("rejected"));
    }

    #[test]
    fn comparison_counts_the_union_of_case_ids() {
        let reference = parse_report(VALID).expect("valid report");
        let mut candidate = reference.clone();
        let mut extra = candidate.cases[0].clone();
        extra.id = "candidate-extra".to_owned();
        candidate.cases.push(extra);

        let summary = compare_reports(&reference, &candidate).expect("compatible reports");
        assert_eq!(summary.case_count, 2);
        assert_eq!(summary.mismatch_count, 1);
        assert_eq!(summary.mismatch_ids, vec!["candidate-extra"]);
    }

    #[test]
    fn report_byte_and_case_bounds_are_enforced() {
        assert_eq!(
            parse_report(&vec![b' '; MAX_REPORT_BYTES + 1]),
            Err(ErrorCode::InvalidEnvelope)
        );
        let mut report = parse_report(VALID).expect("valid report");
        report.cases = vec![report.cases[0].clone(); MAX_CASES + 1];
        assert_eq!(validate_report(&report), Err(ErrorCode::TooManyCases));
    }

    #[test]
    fn invalid_encoding_and_malformed_json_use_fixed_error_codes() {
        assert_eq!(parse_report(&[0xff, 0xfe]), Err(ErrorCode::InvalidEncoding));
        assert_eq!(
            parse_report(br#"{\"private\":\"https://secret.invalid\""#),
            Err(ErrorCode::InvalidJson)
        );
    }
}
