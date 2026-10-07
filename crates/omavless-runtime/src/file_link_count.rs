// SPDX-License-Identifier: MIT

/// Match MetadataExt::nlink without losing bits from either Linux libc width.
/// FileStat uses u32 on ARM64 and u64 on x86_64; the generic conversion also
/// avoids a redundant-conversion lint on the latter.
pub(crate) fn link_count_u64(count: impl Into<u64>) -> u64 {
    count.into()
}

#[cfg(test)]
mod tests {
    use super::link_count_u64;

    #[test]
    fn u32_link_counts_preserve_zero_single_link_and_maximum() {
        for (count, expected) in [(0_u32, 0_u64), (1, 1), (u32::MAX, 4_294_967_295)] {
            assert_eq!(link_count_u64(count), expected);
        }
    }

    #[test]
    fn u64_link_counts_preserve_values_above_u32_and_maximum() {
        for count in [0_u64, 1, 4_294_967_295, 4_294_967_296, u64::MAX] {
            assert_eq!(link_count_u64(count), count);
        }
        assert_ne!(link_count_u64(4_294_967_297_u64), 1);
    }
}
