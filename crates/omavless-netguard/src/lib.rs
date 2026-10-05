//! K1 foundation with an opt-in developer service core. Default builds remain
//! inactive and shipped product runtime does not enable/install the service.
//! The service feature adds fixed private retained-owner effects, not accepted
//! product/host protection or cold-orphan recovery.

#[allow(dead_code)] // Inactive composition; no canonical provider or product caller.
mod authority_composition;
pub mod coordinator;
pub mod effect_port;
pub mod emergency_wire;
mod enrollment;
mod enrollment_provision_candidate;
pub mod full_vpn_wire;
#[cfg(target_os = "linux")]
pub mod kernel_observer;
#[allow(dead_code)] // Inactive acquisition prerequisite; no real constructor.
mod launch_acquisition;
#[allow(dead_code)] // Inactive prerequisite, not an installed socket publisher.
mod listener_admission;
#[allow(dead_code)] // Inactive first publication candidate; no installed service.
mod listener_publisher_candidate;
pub mod locked_state;
#[cfg(test)]
mod manager_config_dump_proposal;
#[cfg(all(test, target_os = "linux"))]
mod manager_config_reference_fixture;
#[cfg(all(test, target_os = "linux"))]
mod manager_version_reference_fixture;
#[cfg(feature = "netguard-service-core")]
pub mod service_core;

#[cfg(all(test, target_os = "linux"))]
mod manager_configured_dump;
#[cfg(all(test, target_os = "linux"))]
mod manager_configured_reference_fixture;
#[cfg(test)]
mod manager_fixture_identity;
#[cfg(test)]
mod manager_lifecycle_admission_dump;
#[cfg(test)]
mod manager_lifecycle_admission_fixture;
#[cfg(test)]
mod manager_lifecycle_permissions;
#[cfg(test)]
mod manager_response_diagnostic_dump;
#[cfg(test)]
mod manager_response_diagnostic_fixture;
#[cfg(test)]
mod manager_response_diagnostic_permissions;

#[cfg(all(test, target_os = "linux"))]
mod manager_negative_witness;
#[cfg(test)]
mod manager_retained_dump;
#[cfg(test)]
mod manager_retained_lifecycle;
#[cfg(test)]
mod manager_retained_reference_fixture;
pub mod nft;
#[allow(dead_code)] // Inactive local package-group identity candidate.
mod package_group_candidate;
pub mod policy;
pub mod protocol;
pub mod receipt;
pub mod receipt_store;
pub mod root_state;
#[allow(dead_code)] // Compiled and tested, but not installed or started.
mod session_owner_candidate;
#[cfg(test)]
#[path = "../../../tests/support/temp.rs"]
mod test_temp;
pub mod transaction;
mod transport_candidate;
