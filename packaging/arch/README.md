# OmaVLESS native package payload

This directory defines the inert filesystem payload for the future Arch
package. The payload contains the prebuilt `omavless` executable, its packaged
systemd user unit, license, third-party notices and this packaging note.

`stage-payload.sh` accepts an existing absolute package build root and an
absolute prebuilt executable. It does not build software, install onto a live
host, invoke a package manager, start or enable a service, or alter OmaVLESS
ownership and private state.

Installing these files alone does not switch VPN ownership. Until the explicit
R5 cutover is accepted, the Omarchy plugin and Python compatibility backend
remain the production owner. Package activation, update, removal and rollback
remain separately reviewed host-integration work.
