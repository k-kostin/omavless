# R5 strict empty-host observation

This checkpoint composes the strict process/TUN inventories with the two fixed
service queries and controller-path absence checks. It is a read-only building
block for future login readiness, not a registered login trigger or permission
to retire Python.

## Admission conditions

Both `omavless.service` and `omavless-runtime.service` must be inactive with zero
main PID. A failed but fully stopped service is not running; this observation
does not declare that service healthy. Active or transitional state, nonzero
PID, invalid/incomplete output and query failure refuse.

The legacy Mihomo socket, native Mihomo socket and native semantic socket must
all be absent. A present but unresponsive socket is not empty. Unsafe links,
ordinary files at socket paths, invalid directory ancestry or unreadable state
refuse. Nothing is deleted or repaired.

Complete bounded process and TUN inventories must return zero. Service, socket
and inventory checks are repeated to catch observed changes; failure never
falls back to the old tolerant zero-on-error projections. No private store or
active configuration is read by this empty-host method.

Fixed service queries have bounded output and a deadline covering both child
termination and pipe completion. A descendant retaining stdout must not make
the reader wait indefinitely. Errors remain fixed public classifications;
raw command output and host/profile details are not returned.

## Limits and future composition

This is an observation, not an atomic kernel reservation or namespace proof.
Future login integration must establish a trusted host process/network view,
hold owner then migration locks, validate exact ownership, call the checks
around isolated candidate validation, and fence inputs before publication.
The helper itself neither acquires those locks nor starts/stops anything.

Remaining work includes exact snapshot-input validation with isolated scratch
config/data, trusted once-per-user-manager ordering, legacy enablement conversion
and packaged activation. #178, #196 and R5/R6 remain separate acceptance gates.

## Reference and acceptance

Python remains the installed startup owner and oracle/rollback. Existing
best-effort observations remain available; the new method intentionally refuses
uncertain data instead of copying zero-on-error behavior. Existing differential
tests protect unchanged projections. No frontend/runtime cutover occurs.
The service-query helper is shared with existing native observation: its
deadline/EOF handling and duplicate-field refusal are hardened by this PR,
even though the new empty-host method has no production caller yet.

Synthetic tests use private temporary filesystem fixtures and a fixed fake
service-query executable. The explicit `OMAVLESS_TEST_EMPTY_HOST=1` integration
test queries real current-user services and reads procfs/sysfs/socket presence;
it requires an already disconnected machine and never changes services, routes,
DNS, configuration or capabilities. Results contain no private identifiers.
This is Try Omarchy ARM64 read-only evidence, not enabled-login or live-provider
acceptance.
