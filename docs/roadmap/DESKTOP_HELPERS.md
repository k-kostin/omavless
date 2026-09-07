# Native desktop helpers — R5 client boundary

Status: bounded native CLI candidate; installed QML/Python path unchanged.

These operations belong to a desktop client, **not the daemon**. They read no
profile store, acquire no ownership marker, start no runtime, change no VPN
state and expose no socket method or arbitrary command executor. The client
obtains explicit private profile/editor/export data through the existing
generation-fenced semantic API, then invokes a fixed helper operation.

## Fixed commands

| Command after `omavless desktop` | Private stdin | Successful stdout |
| --- | --- | --- |
| `capabilities` | none | bounded public helper booleans/provider |
| `clipboard-read` | none | private UTF-8 clipboard text |
| `clipboard-copy` | private UTF-8 text | empty |
| `pick-import` | none | selected file **contents**, never its path |
| `file-read` | absolute local path, optional final newline | private file contents |
| `edit` | explicit UTF-8 editor seed | private edited text |
| `qr` | explicit private profile link | binary PNG |
| `export-file` | absolute destination + newline + explicit private contents | empty |
| `cleanup` | none | public removed-file count |

Unknown commands and extra arguments fail before any helper action. No private
value enters argv or environment; the editor passes only a temporary filename
and a generic title. Successful text/PNG is explicitly private client output,
not a shareable diagnostic. Errors use a fixed bounded English vocabulary and
never echo paths, input fragments or child stderr. The CLI preserves the
installed chooser/editor cancellation convention: exit3 with empty stdout and
stderr. Other failures use the established native exit2 convention. Future QML
composition must treat cancellation as dismissal, not an import failure.

Reading or editing does not classify, fetch, add or replace anything. Feed
acquired content to `import preview`, show the existing confirmation UI, and
only then invoke the relevant semantic mutation. File input and clipboard
input therefore retain one canonical classifier. Editor unchanged-input
detection and confirmation remain client responsibilities. Export preserves
the supplied bytes; profile-link file callers append the established newline.

## Helper selection and bounds

Executable discovery uses only fixed helper names and absolute PATH entries;
it never invokes a shell. Picker preference remains zenity, kdialog, yad.
Configuration editing requires zenity; QR encoding requires qrencode;
clipboard requires wl-clipboard. Missing helpers yield actionable package
names without invoking a package manager or privilege escalation.

**GTK4 fallback is not migrated by this checkpoint.** The existing Python
picker can use `Gtk.FileChooserNative` when none of the three executables is
installed. Native capabilities explicitly report `gtk4FallbackAvailable=false`.
Do not switch installed QML or claim fresh-Omarchy picker parity until a native
GTK/portal implementation or an explicitly accepted dependency policy closes
this gap. No Python subprocess is hidden inside the Rust helper.

### Existing-host fallback assessment

Read-only inspection of Try Omarchy ARM64 on 2026-09-07 found the installed
`QtQuick.Dialogs` QML module, GTK file-chooser portal backend and FileChooser
D-Bus interface specification. The installed Hyprland portal preference is
`hyprland;gtk`, and `gtk.portal` advertises FileChooser. These are available
integration building blocks, **not an exercised chooser acceptance result**.

For the QML frontend, first evaluate its existing QtQuick `FileDialog`: keep
selection/cancellation in the graphical client, pass the selected local path
through private stdin to `desktop file-read`, then reuse semantic preview.
This avoids introducing a GTK Rust dependency into the headless daemon. Verify
native platform/portal selection, panel ownership/modality, close/reopen and
both import paths in the actual shell before replacing Python fallback.

For a later standalone CLI/TUI fallback, a narrow native FileChooser portal
client is preferable to constructing a generic D-Bus/shell proxy. The installed
interface requires an asynchronous request handle and Response signal, not
just a blocking `OpenFile` return. Admission must request single-file selection,
correlate the exact response, preserve explicit cancellation, require exactly
one bounded local `file://` URI and convert it to a validated local path before
reading. `gdbus`/`busctl` presence alone does not supply that complete client.
No dependency or portal implementation is added here. GTK4 bindings are a
third option only after separate dependency/build/host review.

Local reference files: `/usr/lib/qt6/qml/QtQuick/Dialogs/qmldir`,
`/usr/share/xdg-desktop-portal/hyprland-portals.conf`,
`/usr/share/xdg-desktop-portal/portals/gtk.portal`, and
`/usr/share/dbus-1/interfaces/org.freedesktop.portal.FileChooser.xml`.

- Text input/output: 64 KiB, valid UTF-8, no NUL.
- Selected path: absolute UTF-8, 4096 bytes, no control characters or `..`.
- Clipboard: one shared five-second budget, including MIME discovery; prefer
  exact `text/plain`, otherwise preserve wl-paste's default MIME selection.
- MIME listing: 8192 bytes; oversized/unbounded listings fail safely.
- QR: ten seconds, 4 MiB PNG response cap, PNG signature required.
- Interactive chooser/editor: no artificial human-response deadline. Output
  remains bounded and nonzero results never release partial private data.
- Nonblocking simultaneous stdin/stdout prevents pipe deadlocks. Child stderr
  is discarded; failures/timeouts/oversized output kill and reap the owned
  helper process. No general child-process control API is exported.

File reads open regular non-symlink files with no-follow/nonblocking flags to
refuse symlinks, directories and FIFOs without hanging. They accept normal
user-selected readable files, not only private store files.

Exports use a create-exclusive mode-0600 sibling temporary file, synchronize
contents, rename atomically and synchronize the parent directory. The parent
must be a same-user non-symlink directory not writable by group/others; an
existing destination must be a same-user regular non-symlink file. The caller
must obtain destination/overwrite confirmation first. No privileged path or
daemon filesystem API is added.

Editor seeds live in the existing same-user mode-0700 runtime directory, with
mode0600 and unconditional normal/error/cancellation cleanup. No caller-chosen
editor title or command is accepted. `cleanup` only reaps matching private
regular files for dead helper PIDs; it skips live processes, symlinks, unknown
names and unsafe modes, and scans at most4096 entries. Run this fixed cleanup
at frontend startup to recover seeds left after abrupt client termination.
PNG is returned in memory, avoiding an additional persistent credential file;
the eventual frontend must manage its own private image display lifetime.

## Migration evidence and remaining gates

The actual Python `discover_file_picker`/`desktop_helper_status` oracle checks
all eight executable availability combinations with GTK fallback explicitly
disabled for that comparison. Five synthetic clipboard cases execute the
actual shell body extracted from `Service.qml` and compare native status and
digests: plain MIME, default MIME, MIME-list failure, paste failure and empty
clipboard. No real clipboard, display, private store or network is accessed.

Deterministic process/file tests additionally cover size/time bounds, duplex
pipe pressure, cancellation, invalid UTF-8, symlinks/FIFO, shell metacharacters
in literal filenames, editor seed permissions and cleanup, generic title,
QR stdin/PNG validation and atomic export modes. CLI tests exercise actual
fixed commands with an isolated empty helper PATH and absent daemon/socket.
The existing profile export/editor parity corpora remain the private semantic
input reference; this checkpoint does not replace those daemon methods.

Before installed frontend acceptance, run exact-head Try Omarchy helper smoke
with explicit benign clipboard/file content, chooser dismissal/selection,
editor cancel/confirm, actual qrencode output and cleanup. Confirm that the
helper does not alter native/legacy ownership or tunnel state. This evidence
does not require connecting a VPN or unavailable protocol fixtures. Native
GTK parity and QML composition remain separate gates; Python cannot yet be
removed.
