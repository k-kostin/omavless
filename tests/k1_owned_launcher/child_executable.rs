//! Fixed original ELF admission. Pins bind the separately frozen static child.
use super::{identity, require, retain_after, EffectError, ERROR};
use nix::fcntl::{open, openat, OFlag};
use nix::sys::stat::Mode;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::time::Instant;
#[path = "static_elf.rs"]
mod static_elf;
use static_elf::static_elf;

pub(super) const CHILD: &str = "/run/omavless-k1-owned-launch-v3/child";
const SHA: [u8; 32] = [0x78,0x38,0xd1,0xc3,0xb1,0xb2,0x6f,0xa2,0x47,0xd0,0xbb,0x16,0x60,0x81,0x53,0xf5,0x76,0x47,0x7c,0x44,0x28,0x17,0xf8,0xe1,0x52,0x8f,0x6f,0x9c,0x85,0xfe,0x63,0x11];
const SIZE: u64 = 1_475_200;
type Result<T> = std::result::Result<T, EffectError>;

pub(super) struct Executable {
    pub(super) file: ManuallyDrop<File>,
    pub(super) metadata: [u64; 11],
    directories: ManuallyDrop<Vec<(File, [u64; 11])>>,
}
fn gate(end: Instant) -> Result<()> { require(Instant::now() < end) }
fn directory_mode(index: usize, mode: u32) -> bool {
    index < 3 && (mode & 0o7777 == 0o755 || (index == 0 && mode & 0o7777 == 0o555))
}
fn no_attributes(file: &File, end: Instant) -> Result<()> {
    gate(end)?;
    // A fixed initialized byte buffer; nonempty/ERANGE/unsupported all refuse.
    let mut byte = [0_u8; 1];
    let result = rustix::fs::flistxattr(file.as_fd(), &mut byte[..]);
    gate(end)?;
    require(result.map_err(|_| ERROR)? == 0)
}
impl Executable {
    pub(super) fn admit(end: Instant) -> Result<Self> {
        require(SHA != [0; 32] && (64..=16*1024*1024).contains(&SIZE))?;
        gate(end)?;
        let mut directories = ManuallyDrop::new(Vec::<(File,[u64;11])>::with_capacity(3));
        let root = retain_after(open("/", OFlag::O_RDONLY|OFlag::O_DIRECTORY|OFlag::O_NOFOLLOW|OFlag::O_CLOEXEC, Mode::empty())
            .map(File::from),||gate(end).is_ok()).map_err(|_|ERROR)?;
        let mut root = ManuallyDrop::new(Some(ManuallyDrop::into_inner(root)));
        for (index, name) in ["/", "run", "omavless-k1-owned-launch-v3"].iter().enumerate() {
            let directory = if index == 0 {
                ManuallyDrop::new(root.take().ok_or(ERROR)?)
            } else {
                gate(end)?;
                let result = retain_after(openat(&directories[index-1].0, *name,
                    OFlag::O_RDONLY|OFlag::O_DIRECTORY|OFlag::O_NOFOLLOW|OFlag::O_CLOEXEC, Mode::empty())
                    .map(File::from),||gate(end).is_ok());
                result.map_err(|_| ERROR)?
            };
            let held = &*directory;
            gate(end)?; let info = held.metadata(); gate(end)?; let info = info.map_err(|_| ERROR)?;
            require(info.is_dir() && info.uid()==0 && info.gid()==0 && directory_mode(index, info.mode()))?;
            no_attributes(held,end)?;
            directories.push((ManuallyDrop::into_inner(directory),identity(&info)));
        }
        gate(end)?;
        let file = retain_after(openat(&directories[2].0, "child", OFlag::O_RDONLY|OFlag::O_NOFOLLOW|OFlag::O_CLOEXEC|OFlag::O_NONBLOCK,Mode::empty())
            .map(File::from),||gate(end).is_ok()).map_err(|_|ERROR)?;
        require(file.as_raw_fd() >= 3)?; // stdio remapping must not replace it.
        gate(end)?; let info=file.metadata(); gate(end)?; let info=info.map_err(|_|ERROR)?;
        require(info.is_file() && info.uid()==0 && info.gid()==0 && info.mode()&0o7777==0o555
            && info.nlink()==1 && info.size()==SIZE)?;
        let result=Self {file,metadata:identity(&info),directories};
        result.recheck(end)?;
        Ok(result)
    }
    pub(super) fn exec_path(&self) -> String { format!("/proc/self/fd/{}",self.file.as_raw_fd()) }
    pub(super) fn recheck(&self,end:Instant) -> Result<()> {
        for (index,(directory,expected)) in self.directories.iter().enumerate() {
            gate(end)?;let metadata=directory.metadata();gate(end)?;
            require(identity(&metadata.map_err(|_|ERROR)?)==*expected)?;
            let path=["/","/run","/run/omavless-k1-owned-launch-v3"][index];
            gate(end)?;let metadata=std::fs::symlink_metadata(path);gate(end)?;
            require(identity(&metadata.map_err(|_|ERROR)?)==*expected)?;
            no_attributes(directory,end)?;
        }
        gate(end)?;let metadata=self.file.metadata();gate(end)?;
        require(identity(&metadata.map_err(|_|ERROR)?)==self.metadata)?;
        no_attributes(&self.file,end)?;
        let mut bytes=vec![0_u8;usize::try_from(SIZE).map_err(|_|ERROR)?];
        let mut offset=0;
        while offset<bytes.len() {
            gate(end)?;let count=self.file.read_at(&mut bytes[offset..],offset as u64);gate(end)?;
            let count=count.map_err(|_|ERROR)?;require(count>0 && count<=bytes.len()-offset)?;offset+=count;
        }
        gate(end)?;let count=self.file.read_at(&mut [0_u8;1],SIZE);gate(end)?;
        require(count.map_err(|_|ERROR)?==0)?;
        require(static_elf(&bytes) && <[u8;32]>::from(Sha256::digest(&bytes))==SHA)?;
        gate(end)?;let metadata=self.file.metadata();gate(end)?;
        require(identity(&metadata.map_err(|_|ERROR)?)==self.metadata)?;
        gate(end)?;let metadata=std::fs::symlink_metadata(CHILD);gate(end)?;
        require(identity(&metadata.map_err(|_|ERROR)?)==self.metadata)?;
        // Ancestry is sampled again after the full original-byte hash, not
        // inferred from a matching leaf pathname alone.
        for (index,(directory,expected)) in self.directories.iter().enumerate() {
            gate(end)?;let metadata=directory.metadata();gate(end)?;
            require(identity(&metadata.map_err(|_|ERROR)?)==*expected)?;
            let path=["/","/run","/run/omavless-k1-owned-launch-v3"][index];
            gate(end)?;let metadata=std::fs::symlink_metadata(path);gate(end)?;
            require(identity(&metadata.map_err(|_|ERROR)?)==*expected)?;
        }
        gate(end)
    }
}

#[cfg(test)]
mod mode_controls {
    use super::directory_mode;
    #[test]
    fn root_readonly_is_closed_and_other_ancestors_stay_exact() {
        for index in 0..4 {
            for mode in [0o555, 0o755, 0o711, 0o700, 0o775, 0o777, 0o1555, 0o4755] {
                assert_eq!(directory_mode(index, mode),
                    index < 3 && (mode == 0o755 || (index == 0 && mode == 0o555)));
            }
        }
    }
}
