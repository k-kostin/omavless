# Review-2 transport successor — source only

The sealed `74a637a` generation passed its source gates, but staging stopped
before any wrapper invocation: the transfer command supplied an invalid dd
output flag. Fresh canonical epoch preflight and new-stage creation completed;
the first file transfer returned known exit 1. There were zero wrapper attempts
and no further guest queries, reads, cleanup, export or retry. The original
review-1 stage remains retained and uninspected. This is transport NONPASS, not
actual inventory or compatibility evidence.

Every file in `../live_fd_tmpfs/` remains unchanged. This distinct generation
changes only the literal stage to `t3-live-fd-tmpfs-review-2` and resulting
probe/validator/wrapper hash wiring. It reuses the exact bridge, admission,
manifest and containment sources. The predecessor's full scope and terminal
unknown-state contracts still apply. No resolved/broker/core/ELF admission or
supervision behavior is changed.

`transport.py` is a separately reviewed fixed staging helper, not a ninth
archived/staged fixture member. Trusted host code passes this exact helper to
guest Python and supplies only the selected fixed artifact bytes on stdin.
The only commands are `--create-stage` with empty stdin and `--put` with one
of eight literal basenames. It validates bounded payload size and exact hash
before opening directories or creating files. All ancestry components are
opened no-follow through retained directory FDs; private stage ownership/mode,
finite member names and final directory identity checks are mandatory.

Stage creation uses exclusive mkdir; payload creation uses O_CREAT|O_EXCL,
never truncation or overwrite. One write must transfer all bytes; short or
unknown writes refuse and retain the partial new file, without resume/unlink.
The same new original FD is mode-checked, fsynced, hashed and matched to its
path before single-attempt closure. No payload is imported or executed.
Host orchestration must stop on the first failed/unknown transfer, and never
invoke a wrapper unless all eight transfers and exact wrapper pin checks pass.
Staging itself needs a fresh explicit lease and reviewed immutable helper.

Native synthetic controls execute actual O_EXCL writes inside isolated temporary
directories, covering success/modes, existing target/stage, wrong names/hash/
size, partial write, symlink, unknown member, unsafe ancestor and unknown stat.
These are local source tests, not guest access. All eight strict launcher/
receipt/terminal-wrapper controls are repeated for this exact generation.

The success-only exporter is a separate private reviewed source artifact, not
part of normal runtime or this fixture's execution. Its review-2 version changes
only stage and three resulting source hashes; its exact 28-member/private
original-FD checks remain required. Export still needs independent approval
after known whole-wrapper success, never merely successful staging.
