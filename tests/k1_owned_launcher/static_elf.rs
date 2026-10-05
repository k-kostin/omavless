//! Fixed ELF64 little-endian x86-64 structural admission, not a loader emulator.
//! Whole-file exact SHA separately binds all remaining bytes and segment policy.
pub(super) fn static_elf(bytes:&[u8])->bool {
    if bytes.len()<64 || &bytes[..7]!=b"\x7fELF\x02\x01\x01" {return false;}
    let u16at=|n|u16::from_le_bytes([bytes[n],bytes[n+1]]);
    let u64at=|n|u64::from_le_bytes(bytes[n..n+8].try_into().unwrap());
    if !matches!(u16at(16),2|3) || u16at(18)!=62 || u16at(52)!=64 || u16at(54)!=56 {return false;}
    let count=usize::from(u16at(56));let Ok(start)=usize::try_from(u64at(32))else{return false;};
    if !(1..=64).contains(&count) || start<64 || start.checked_add(count*56).is_none_or(|end|end>bytes.len()) {return false;}
    let kinds:Vec<_>=(0..count).map(|index|u32::from_le_bytes(bytes[start+index*56..start+index*56+4].try_into().unwrap())).collect();
    kinds.contains(&1) && !kinds.contains(&3) && bytes[20..24]==[1,0,0,0]
}
#[cfg(test)]
mod tests {
    use super::*;
    fn example()->Vec<u8> {
        let mut bytes=vec![0;120];bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        for (at,value) in [(16,2_u16),(18,62),(52,64),(54,56),(56,1)] {bytes[at..at+2].copy_from_slice(&value.to_le_bytes());}
        bytes[20]=1;bytes[32]=64;bytes[64]=1;bytes
    }
    #[test]
    fn exact_arch_static_load_and_no_interpreter() {
        let mut bytes=example();assert!(static_elf(&bytes));bytes[16]=3;assert!(static_elf(&bytes));
        bytes[64]=3;assert!(!static_elf(&bytes));bytes[64]=0;assert!(!static_elf(&bytes));
        for at in [4,5,6,16,18,20,52,54,56] {let mut bytes=example();bytes[at]=0;assert!(!static_elf(&bytes));}
    }
    #[test]
    fn every_truncation_and_extended_overflow_refuses_without_panic() {
        let bytes=example();for length in 0..bytes.len(){assert!(!static_elf(&bytes[..length]));}
        for count in [0_u16,65,u16::MAX] {let mut bytes=example();bytes[56..58].copy_from_slice(&count.to_le_bytes());assert!(!static_elf(&bytes));}
        for offset in [0_u64,63,65,u64::MAX] {let mut bytes=example();bytes[32..40].copy_from_slice(&offset.to_le_bytes());assert!(!static_elf(&bytes));}
    }
}
