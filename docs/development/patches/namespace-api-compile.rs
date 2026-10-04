#![forbid(unsafe_code)]
//! Compile-only external API and Linux generic-ABI witness, not namespace authority.
use std::os::fd::{AsFd, BorrowedFd};
use nix::{Result, sys::{nsfs::{namespace_id, namespace_type, NamespaceType}, socket::{GetSockOpt, sockopt::NetnsCookie}}};

pub fn borrowed_kind(fd: BorrowedFd<'_>) -> Result<NamespaceType> { namespace_type(fd) }
pub fn borrowed_id(fd: BorrowedFd<'_>) -> Result<u64> { namespace_id(fd) }
pub fn socket_cookie<F: AsFd>(fd: &F) -> Result<u64> { NetnsCookie.get(fd) }

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const _: [(); 1] = [(); (nix::request_code_read!(0xb7, 13, 8) == 0x8008b70d) as usize];
const _: [(); 8] = [(); std::mem::size_of::<u64>()];
const _: [(); 8] = [(); std::mem::align_of::<u64>()];
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const _: [(); 1] = [(); (nix::libc::NS_GET_ID as u64 == 0x8008b70d) as usize];
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const _: [(); 1] = [(); (libc_main::NS_GET_ID as u64 == 0x8008b70d) as usize];

/// ```compile_fail,E0505
/// use std::{fs::File, os::fd::AsFd};
/// let owner = File::open("/dev/null").unwrap();
/// let original = owner.as_fd();
/// drop(owner);
/// let _ = namespace_api_compile_review::borrowed_id(original);
/// ```
/// ```compile_fail,E0505
/// use std::{fs::File, os::fd::AsFd};
/// let owner = File::open("/dev/null").unwrap();
/// let original = owner.as_fd();
/// drop(owner);
/// let _ = namespace_api_compile_review::socket_cookie(&original);
/// ```
pub struct LifetimeChecks;
