// SPDX-License-Identifier: MIT
// Source identities precede distribution link rewriting. A human review is
// recorded only after the owner has read this exact English/Russian pair.
// BEGIN GUIDE MANIFEST
var guideManifest = {
  "schemaVersion": 1,
  "version": "0.9.9",
  "english": {
    "path": "docs/user/NATIVE_INSTALL.md",
    "sha256": "20464ca1574e0e517fcc9c05a95fde6235574ea347ed504c2b028ed847aa1014"
  },
  "russian": {
    "path": "docs/user/NATIVE_INSTALL_RU.md",
    "sha256": "47824f4fe895466664ba65d3ccafcaf8fab1a355ad1b5440b0d505e9540bba1e",
    "humanReview": null
  }
};
// END GUIDE MANIFEST

function russianReady(version) {
  var m = guideManifest
  if (!m || m.schemaVersion !== 1 || m.version !== version || !m.english || !m.russian) return false
  if (m.english.path !== "docs/user/NATIVE_INSTALL.md" || m.russian.path !== "docs/user/NATIVE_INSTALL_RU.md") return false
  if (!/^[0-9a-f]{64}$/.test(m.english.sha256) || !/^[0-9a-f]{64}$/.test(m.russian.sha256)) return false
  var r = m.russian.humanReview
  return !!r && r.kind === "owner-human-meaning" && r.version === version
    && r.englishSha256 === m.english.sha256 && r.russianSha256 === m.russian.sha256
    && typeof r.reviewedOn === "string" && /^\d{4}-\d{2}-\d{2}$/.test(r.reviewedOn)
}

function guideUrl(locale, version, section) {
  var anchor = section === "install" || section === "updates" ? section : "help"
  var russian = locale === "ru" && russianReady(version)
  var page = russian ? "NATIVE_INSTALL_RU.md" : "NATIVE_INSTALL.md"
  return "https://github.com/k-kostin/omavless/blob/main/docs/user/" + page + "#" + anchor
}

if (typeof module !== "undefined") module.exports = {guideUrl:guideUrl, russianReady:russianReady, manifest:guideManifest}
