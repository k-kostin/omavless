# K1 manager handoff: pinned source audit, not origin authority

This is a source-only audit accompanying binder checkpoint
`686721531687caf45387d4f6b41a18460e48af8b`. No real namespace ioctl, netlink
constructor, service invocation or VM acceptance was performed for this note.
Neither a matching namespace nor a systemd-shaped launch authenticates the
canonical manager. Product dependency adoption remains unauthorized.

## Kernel identity comparison

The comparison uses the full `u64` namespace identifier, not namespace inode
numbers and not `SO_COOKIE`. At Linux commit
`a90ee4305c4a5df72c11b31dacfdc76e00fcf78a`, the implementation chain is:

1. [`__ns_tree_gen_id`](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/kernel/nstree.c)
   assigns `ns->ns_id` and returns that same field.
2. [`setup_net`](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/net/core/net_namespace.c)
   stores the result of `ns_tree_gen_id(net)` in `net->net_cookie`; the macro in
   `include/linux/nstree.h` passes that network namespace's `ns_common`.
3. [`NS_GET_ID`](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/fs/nsfs.c)
   returns `ns->ns_id` through a `u64` output pointer.
4. [`SO_NETNS_COOKIE`](https://github.com/torvalds/linux/blob/a90ee4305c4a5df72c11b31dacfdc76e00fcf78a/net/core/sock.c)
   requires an eight-byte output and returns `sock_net(sk)->net_cookie`.
   The separate `SO_COOKIE` branch calls `sock_gen_cookie(sk)` instead.

Thus the retained network-namespace descriptor and actual retained creator
socket can be compared by these APIs. Unsupported kernels still refuse; this
source comparison is not evidence that a particular guest implements the API.

## Actual opening and transfer path

The inspected systemd release is v261, peeled commit
`de9dbc37ad4aa637e200ac02a0545095997055df` (annotated tag object
`102d5065bc82a875cfa0f6fcae6a5bda651cbf0a`). Read-only upstream metadata also
pinned current HEAD `d404b58f3a6ee2af7267684f68e348686f8ac161`.

The release's [`exec_spawn`](https://github.com/systemd/systemd/blob/de9dbc37ad4aa637e200ac02a0545095997055df/src/core/execute.c)
serializes the invocation and starts the manager-selected executor through its
retained executable descriptor. The reached `posix_spawn_wrapper` in
`src/basic/process-util.c` configures signal-mask/cgroup placement, not a
namespace transition. The executor deserializes the invocation and enters
`exec_invoke`; its name or command-line serialization is not an authentication
token, and a caller can launch an executor of its own.

In [`exec-invoke.c`](https://github.com/systemd/systemd/blob/de9dbc37ad4aa637e200ac02a0545095997055df/src/core/exec-invoke.c),
`collect_open_file_fds` runs before the later service user/network namespace
setup. `get_open_file_fd` first opens the configured path with `O_PATH`, checks
its type, and reopens a non-socket object with the selected access flags.
The proposed namespace handoff must select only `read-only`, without
`graceful`, append or truncate. Socket paths take a different connect path and
are outside this proposal. `/proc/self/ns/net` here refers to the executor at
the opening operation, before the service's subsequent sandbox setup; it does
not mean PID 1 and does not independently establish a canonical host origin.

[`fd_reopen` and `pack_fds`](https://github.com/systemd/systemd/blob/de9dbc37ad4aa637e200ac02a0545095997055df/src/basic/fd-util.c)
were read in full: the former opens the original descriptor's procfs path;
the latter duplicates/renumbers the retained descriptors consecutively from
3. After packing, `flag_fds` clears close-on-exec for the passed descriptors,
and the final executable handoff preserves them. Reopening an inherited
namespace descriptor produces another descriptor for the namespace object;
it must not be described as ownership of the same open file description.
Named standard-I/O resolution precedes OpenFile collection in this path, so
this audit does not assume `StandardInput=fd:<new-OpenFile-name>` works.

The complete v261-to-pinned-HEAD diff of `exec-invoke.c` and `executor.c` was
read. It preserves the relevant OpenFile-before-namespace and packing/exec
ordering. The release's reached spawn and FD helpers were inspected; this is
not a full audit of every current systemd source, serializer, libc spawn
implementation, effective unit, installed package or running manager.

## Next bounded prototype and remaining obligation

A fresh noninstalled fixed service can test this opening order with the
isolated patched-library binder: retain the handed-off namespace object,
compare it to the calling thread and its actual original netlink socket,
then recheck without sending a datagram. A separately isolated network
namespace should produce refusal. Such a prototype requires its own exact
source/build/freeze/transport review and ROOT-only invocation; none is
authorized by this document alone.

Production still requires a trusted installed package and effective unit,
exclusive system-manager launch and descriptor provenance before sandboxing,
and a reviewed safe ownership/adoption path into the existing acquisition
session. Root UID, PID 1, descriptor numbers/names, LISTEN metadata, copied
IDs and this local equality cannot manufacture that authority.

## Scoped binder evidence

At exact binder 6867215, fresh source gates passed 687 tests with two skips
plus JS/QML checks. The isolated Rust 1.69 x86-64 gate passed nine native
synthetic controls and three compile-fail doctests; ARM64 was compile-only.
ROOT independently reran nine plus three using a separate target and found
no scoped source blocker. These tests use synthetic leaf operations, ordinary
files and own-process descriptor metadata, not real namespace/socket queries.
No earlier private lifecycle VM acceptance transfers to this binder.
