# Private test directories

`temp.rs` is a shared std-only test module, included only under `cfg(test)`.
It allocates short, initially mode-0700 directories with exclusive creation,
a process ID and atomic sequence. Existing directories, files and symlinks are
never reused; collisions retry up to 128 times. Wall-clock precision is not an
identity guarantee, particularly on the ARM64 VM.

The first conversion covers routing preset, production owner, production
cutover, private-store transaction and Mihomo observation fixtures. These
previously created timestamp-named roots with `create_dir_all`, which could
silently share state after a collision. Other helpers are intentionally not
rewritten in this bounded checkpoint.

Keep fixture files and symlinks beneath the returned directory. Use
`create_new` when testing a new regular file. The caller owns cleanup after
joining its threads and waiting for children; this helper does not introduce
automatic deletion while work might still be running. Nested `create_dir_all`
is safe only after the root has been exclusively allocated.

Two earlier VM runs exposed this class of issue: timestamp-only store fixture
allocation failed, and concurrent parity file/symlink tests aliased a path,
modifying a checked-in synthetic reference. Their immediate fixes remain
preserved. No private application data was involved.

The shared helper tests cover parallel uniqueness/private mode, forced existing
directory and live/dangling symlink collisions, unchanged external targets,
bounded exhaustion and rejected path-like labels. They run with both consuming
crate unit suites, or independently with:

```sh
rustc --edition=2024 --test tests/support/temp.rs -o /tmp/omavless-temp-tests
/tmp/omavless-temp-tests
```

No production code path, application dependency or installed runtime changes.
