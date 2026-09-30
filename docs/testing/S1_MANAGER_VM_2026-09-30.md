# S1 read-only manager continuity in Omarchy Dev VM

This is a scoped development smoke, not App proxy acceptance. Source was
`0dd9f255364332aa6bce1153d47a45585b502553` on the S1 manager-probe
branch. The x86_64 Omarchy Dev VM ran only the optional
`omavless-s1-manager-continuity-probe` binary built with
`manager-continuity-probe`; binary SHA-256 was
`a227d98043058029fa8de1a01b0ffc1ab9ecf1efb5b054fa1227a7b98bd3b337`.
The transferred executable was removed after the test.

The normal bounded invocation exited 0 with exactly
`manager_continuity=observed_unverified`. An ambient system-bus override to a
different socket and `G_DBUS_DEBUG=all` each exited 1 with exactly
`manager_continuity=refused`. An extra argument exited 2 without output.
Local feature-enabled unit tests, strict crate Clippy and binary build passed.
These checks neither read/write the manager environment nor modify desktop
proxy settings, OmaVLESS services, TUN, routes or the active VPN connection.

The positive result means only that the current guest's fixed system/user
manager endpoint observations met the narrow read-only continuity predicates
at that instant. It does not authenticate the process serving future requests,
bind a graphical/UWSM session, prove the broker's activation environment,
eliminate an effect-time race or authorize a writable S1 adapter. App proxy
remains unavailable. The unresolved authority and restoration gates are in
[the S1 manager diagnostic](../development/S1_MANAGER_CONTINUITY_PROBE.md)
and [foundation contract](../development/S1_PROXY_FOUNDATION.md). No
Try Omarchy ARM64, installed App proxy or physical-PC acceptance is claimed.
