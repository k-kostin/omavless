//! Original return ownership is established BEFORE late/error/panic classification.
use std::mem::ManuallyDrop;

pub(super) fn retain_after<T,E>(result:Result<T,E>, check:impl FnOnce()->bool)->Result<ManuallyDrop<T>,()> {
    let original=result.map(ManuallyDrop::new);
    if !check() {return Err(());}
    original.map_err(|_|())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::AsRawFd;
    #[test]
    fn actual_returned_file_stays_open_after_late_and_panicking_gate() {
        for panic in [false,true] {
            let file=File::open("/dev/null").unwrap();let fd=file.as_raw_fd();
            let before=file.metadata().unwrap();
            let result=std::panic::catch_unwind(||retain_after(Ok::<_,()>(file),|| {
                if panic {panic!("synthetic late gate");} false
            }));
            if panic {assert!(result.is_err());}else{assert!(result.unwrap().is_err());}
            let after=std::fs::metadata(format!("/proc/self/fd/{fd}")).unwrap();
            use std::os::unix::fs::MetadataExt;
            assert_eq!((before.dev(),before.ino()),(after.dev(),after.ino()));
        }
    }
    #[test]
    fn existing_failure_never_turns_into_owner_or_success() {
        for timely in [false,true] {
            let mut calls=0;
            assert!(retain_after::<File,_>(Err(()),||{calls+=1;timely}).is_err());
            assert_eq!(calls,1);
        }
    }
    #[test]
    fn timely_original_is_not_replaced_or_duplicated() {
        let file=File::open("/dev/null").unwrap();let fd=file.as_raw_fd();
        let held=retain_after(Ok::<_,()>(file),||true).unwrap();
        assert_eq!(held.as_raw_fd(),fd);
        // Only this known successful synthetic owner is deliberately released.
        drop(ManuallyDrop::into_inner(held));
    }
}
