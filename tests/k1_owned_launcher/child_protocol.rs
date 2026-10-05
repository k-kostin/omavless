//! Fixed private pipe protocol. No namespace, creator, path or authority data.
use std::io::{self, Read, Write};
use std::cell::Cell;

pub(super) const READY: &[u8] = b"K1_CHILD_READY\n";
pub(super) const FINISH: &[u8] = b"K1_CHILD_FINISH\n";
pub(super) const DONE: &[u8] = b"K1_CHILD_DONE\n";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Refused;
pub(super) type Result<T> = std::result::Result<T, Refused>;

pub(super) struct ReadyState {attempted:Cell<bool>,complete:Cell<bool>}
impl ReadyState {
    pub(super) fn new()->Self {Self{attempted:Cell::new(false),complete:Cell::new(false)}}
    pub(super) fn receive(&self,reader:&mut impl Read,gate:&mut impl FnMut()->Result<()>)->Result<()> {
        if self.attempted.replace(true) {return Err(Refused);}
        read_frame(reader,READY,gate)?;self.complete.set(true);Ok(())
    }
    pub(super) fn complete(&self)->bool {self.complete.get()}
}

// These functions are private to the fixed executable/parent. A caller cannot
// inject a clock or I/O provider through any acquisition or executable API.
// Only WouldBlock permits another attempt. Every returned leaf is fenced;
// errors, zero/overlarge writes and unexpected EOF permanently fail the caller.
pub(super) fn write_frame(
    writer: &mut impl Write, frame: &[u8], gate: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut offset = 0;
    while offset < frame.len() {
        gate()?;
        let result = writer.write(&frame[offset..]);
        gate()?;
        match result {
            Ok(count) if count > 0 && count <= frame.len() - offset => offset += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::hint::spin_loop(),
            _ => return Err(Refused),
        }
    }
    gate()?;
    let result = writer.flush();
    gate()?;
    result.map_err(|_| Refused)
}

pub(super) fn read_frame(
    reader: &mut impl Read, frame: &[u8], gate: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if frame.is_empty() || frame.len() > 32 { return Err(Refused); }
    let mut bytes = [0_u8; 33];
    let mut offset = 0;
    while offset < frame.len() {
        gate()?;
        // One extra byte detects a same-read suffix instead of discarding it.
        let result = reader.read(&mut bytes[offset..frame.len() + 1]);
        gate()?;
        match result {
            Ok(count) if count > 0 && count <= frame.len() - offset => offset += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::hint::spin_loop(),
            _ => return Err(Refused),
        }
    }
    if &bytes[..offset] != frame { return Err(Refused); }
    gate()
}

pub(super) fn idle(reader: &mut impl Read, gate: &mut impl FnMut() -> Result<()>) -> Result<()> {
    gate()?;
    let result = reader.read(&mut [0_u8; 1]);
    gate()?;
    match result {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        _ => Err(Refused),
    }
}

pub(super) fn eof(reader: &mut impl Read, gate: &mut impl FnMut() -> Result<()>) -> Result<()> {
    gate()?;
    let result = reader.read(&mut [0_u8; 1]);
    gate()?;
    match result { Ok(0) => Ok(()), _ => Err(Refused) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Pipe { reads: VecDeque<io::Result<Vec<u8>>>, writes: Vec<u8>, flushes: usize }
    impl Read for Pipe {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let bytes = self.reads.pop_front().expect("no unexpected read")?;
            assert!(bytes.len() <= out.len()); out[..bytes.len()].copy_from_slice(&bytes); Ok(bytes.len())
        }
    }
    impl Write for Pipe {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.writes.extend(bytes); Ok(bytes.len()) }
        fn flush(&mut self) -> io::Result<()> { self.flushes += 1; Ok(()) }
    }
    fn pipe(reads: Vec<io::Result<Vec<u8>>>) -> Pipe { Pipe { reads: reads.into(), writes: vec![], flushes: 0 } }
    fn blocked() -> io::Result<Vec<u8>> { Err(io::ErrorKind::WouldBlock.into()) }
    #[test]
    fn actual_fixed_frames_split_idle_and_eof() {
        let mut p = pipe(vec![blocked(), Ok(READY[..3].to_vec()), Ok(READY[3..].to_vec()), blocked(), Ok(DONE.to_vec()), Ok(vec![])]);
        let mut gate = || Ok(());
        read_frame(&mut p, READY, &mut gate).unwrap(); idle(&mut p, &mut gate).unwrap();
        write_frame(&mut p, FINISH, &mut gate).unwrap();
        read_frame(&mut p, DONE, &mut gate).unwrap(); eof(&mut p, &mut gate).unwrap();
        assert_eq!(p.writes, FINISH); assert_eq!(p.flushes, 1);
    }
    #[test]
    fn wrong_suffix_eof_and_interrupted_are_not_retryable() {
        for bytes in [vec![], b"K1_CHILD_WRONG\n".to_vec(), [READY,b"x"].concat()] {
            let mut p=pipe(vec![Ok(bytes)]); assert!(read_frame(&mut p,READY,&mut ||Ok(())).is_err());
        }
        let mut p=pipe(vec![Err(io::ErrorKind::Interrupted.into())]);
        assert!(read_frame(&mut p,READY,&mut ||Ok(())).is_err());
        for bytes in [vec![], vec![1]] { assert!(idle(&mut pipe(vec![Ok(bytes)]),&mut ||Ok(())).is_err()); }
    }
    #[test]
    fn late_return_stops_before_next_read_or_flush() {
        for late in 1..=2 {
            let mut calls=0;
            let mut gate=||{calls+=1; if calls==late {Err(Refused)} else {Ok(())}};
            let mut p=pipe(vec![Ok(READY.to_vec())]);
            assert!(read_frame(&mut p,READY,&mut gate).is_err());
            assert_eq!(p.reads.len(),usize::from(late==1));
        }
        let mut calls=0;
        let mut gate=||{calls+=1;if calls==2 {Err(Refused)}else{Ok(())}};
        let mut p=pipe(vec![]);assert!(write_frame(&mut p,FINISH,&mut gate).is_err());
        assert_eq!(p.writes,FINISH);assert_eq!(p.flushes,0);
    }
    #[test]
    fn late_flush_refuses_without_second_attempt() {
        let mut calls=0;
        let mut gate=||{calls+=1;if calls==4 {Err(Refused)}else{Ok(())}};
        let mut p=pipe(vec![]);assert!(write_frame(&mut p,FINISH,&mut gate).is_err());
        assert_eq!(p.flushes,1);assert_eq!(p.writes,FINISH);
    }
    #[test]
    fn ready_all_gate_errors_panics_and_received_refusals_permanently_latch() {
        use std::panic::{catch_unwind,AssertUnwindSafe};
        for cut in 0..3 {for panic in [false,true] {
            let state=ReadyState::new();let mut p=pipe(vec![Ok(READY.to_vec())]);let mut gates=0;
            let result=catch_unwind(AssertUnwindSafe(||state.receive(&mut p,&mut ||{
                let at=gates;gates+=1;if at==cut {if panic {panic!("inert READY gate");}return Err(Refused);}Ok(())
            })));
            if panic {assert!(result.is_err());}else{assert_eq!(result.unwrap(),Err(Refused));}
            assert!(!state.complete());let remaining=p.reads.len();
            assert_eq!(state.receive(&mut p,&mut ||panic!("retry")),Err(Refused));assert_eq!(p.reads.len(),remaining);
        }}
        for result in [Ok(vec![]),Ok([READY,b"x"].concat()),Err(io::ErrorKind::Interrupted.into()),Err(io::ErrorKind::Other.into())] {
            let state=ReadyState::new();let mut p=pipe(vec![result]);
            assert_eq!(state.receive(&mut p,&mut ||Ok(())),Err(Refused));assert!(!state.complete());
            assert_eq!(state.receive(&mut p,&mut ||panic!("retry")),Err(Refused));
        }
        let state=ReadyState::new();let mut p=pipe(vec![Ok(READY.to_vec())]);
        assert_eq!(state.receive(&mut p,&mut ||Ok(())),Ok(()));assert!(state.complete());
        assert_eq!(state.receive(&mut p,&mut ||panic!("repeat")),Err(Refused));
    }
}
