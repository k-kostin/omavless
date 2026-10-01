// SPDX-License-Identifier: MIT

//! A surviving private transaction is an ambiguity fence, regardless of
//! which internal operation created it. Never infer that it is safe to retry,
//! connect, or stop from the marker's contents alone.

use crate::desired::DesiredPaths;

pub(crate) fn pending(paths: &DesiredPaths) -> bool {
    crate::routing_preset::pending(paths)
        || crate::restore_staging_candidate::staging_pending(paths)
}
