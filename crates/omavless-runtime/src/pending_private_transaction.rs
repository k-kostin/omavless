// SPDX-License-Identifier: MIT

//! A surviving private transaction is an ambiguity fence, regardless of
//! which internal operation created it. Never infer that it is safe to retry,
//! connect, or stop from the marker's contents alone.

use crate::desired::DesiredPaths;
use std::path::Path;

pub(crate) fn pending(paths: &DesiredPaths) -> bool {
    pending_at(&paths.directory)
}

/// The fixed state directory is also available to startup/cutover callers
/// before they construct a desired-state owner. This remains an existence
/// fence: inaccessible, malformed and unexpected entry types all block.
pub(crate) fn pending_at(directory: &Path) -> bool {
    crate::routing_preset::pending_at(directory)
        || crate::restore_staging_candidate::staging_pending_at(directory)
}
