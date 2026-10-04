# SPDX-License-Identifier: MIT
"""Experimental seccomp filter for the namespace-only TUN ownership probe.

Not a production sandbox: only native aarch64/x86_64 are modeled. It removes
ioctl-based TUN administration while retaining TUNGETIFF, read-only TCGETS and
descriptor-only FIOCLEX required by libc/Python. All io_uring entry
points and sendmsg/sendmmsg are refused: no alternate command submission or
SCM_RIGHTS export to an unfiltered consumer. This also affects ordinary UDP
ancillary sends and therefore needs separate real packet-path compatibility.
Review full runtime syscall needs before considering product integration.
"""
import errno
import struct

ARCHITECTURES = {
    'aarch64': (0xC00000B7, 29),
    'x86_64': (0xC000003E, 16),
}
FD_SEND_SYSCALLS = {'aarch64': (211, 269), 'x86_64': (46, 307)}
ALLOW = 0x7FFF0000
DENY = 0x00050000 | errno.EPERM
KILL = 0x80000000
TUNGETIFF = 0x800454D2


def instructions(machine):
    if machine not in ARCHITECTURES:
        raise ValueError('unsupported_probe_architecture')
    arch, ioctl = ARCHITECTURES[machine]
    sendmsg, sendmmsg = FD_SEND_SYSCALLS[machine]
    return [
        (0x20, 0, 0, 4),              # load seccomp_data.arch
        (0x15, 1, 0, arch),
        (0x06, 0, 0, KILL),           # refuse compat/foreign ABI
        (0x20, 0, 0, 0),              # load syscall number
        (0x45, 0, 1, 0x40000000),     # refuse x32 ABI as well
        (0x06, 0, 0, DENY),
        (0x15, 0, 1, 425),           # io_uring_setup
        (0x06, 0, 0, DENY),
        (0x15, 0, 1, 426),           # io_uring_enter
        (0x06, 0, 0, DENY),
        (0x15, 0, 1, 427),           # io_uring_register
        (0x06, 0, 0, DENY),
        (0x15, 0, 1, sendmsg),       # includes SCM_RIGHTS descriptor export
        (0x06, 0, 0, DENY),
        (0x15, 0, 1, sendmmsg),
        (0x06, 0, 0, DENY),
        (0x15, 1, 0, ioctl),
        (0x06, 0, 0, ALLOW),
        (0x20, 0, 0, 24),             # ioctl request, low u32 of args[1]
        (0x15, 3, 0, TUNGETIFF),
        (0x15, 2, 0, 0x5401),         # TCGETS: libc/Python isatty, read-only
        (0x15, 1, 0, 0x5451),         # FIOCLEX: libc close-on-exec
        (0x06, 0, 0, DENY),
        (0x06, 0, 0, ALLOW),
    ]


def encode(machine):
    return b''.join(struct.pack('=HBBI', *item) for item in instructions(machine))
