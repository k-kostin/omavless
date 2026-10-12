// SPDX-License-Identifier: MIT
// Match the Shell research-navigation gate. Widget chords are not list actions.
pub fn plain_key(key: &str, modified: bool) -> Option<&str> {
    (!modified).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_key_admission() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../keyboard-fixtures.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let key = case["key"].as_str().unwrap();
            let modified = case["modified"].as_bool().unwrap();
            assert_eq!(plain_key(key, modified), case["plain"].as_str());
        }
    }
}
