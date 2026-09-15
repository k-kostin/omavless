// SPDX-License-Identifier: MIT
// No store, subprocess, network, credentials, or connected-state fixture.
import {pathToFileURL} from 'node:url';
export const revision = 7;
const instanceId = 'marketing-demo';
const desired = {connected:false, profileId:'', mode:'global', generation:1};
const profile = (id, name, subscriptionId = '', favorite = false) =>
  ({id, name, subscriptionId, favorite, protocol:'vless', missing:false});
export function response(command) {
  const envelope = result => ({api:'omavless.control', version:1, id:'demo', ok:true, revision, result});
  switch (command) {
    case 'status': return envelope({
      schemaVersion:1, scope:'private_ui_metadata', instanceId, transition:null,
      desired, lastKnownActual:'disconnected', healthFresh:false, liveHealth:'unavailable',
      profiles:[profile('demo-personal','Demo Personal', '', true),
        profile('demo-backup','Demo Backup'),
        profile('demo-nl','Demo Netherlands','demo-subscription'),
        profile('demo-de','Demo Germany','demo-subscription'),
        profile('demo-fr','Demo France','demo-subscription')],
      subscriptions:[{id:'demo-subscription', name:'Demo Subscription', updatedAt:0, profileCount:3, staleCount:0}],
      lastProfileId:'', startup:{configured:true, enabled:false, target:'last', profileId:'', mode:'rule'},
      onboardingComplete:true, routing:{storedPreset:'roscomvpn-default', customRuleCount:0}
    });
    case 'native-observation': return envelope({
      schemaVersion:1, scope:'local_runtime_observation', instanceId, transition:null,
      availability:'observed', desired:{connected:false, mode:'global', generation:1},
      lastKnownActual:'disconnected', manualRecoveryRequired:false,
      facts:{ownedCoreRunning:false, visibleMihomoCount:0, visibleTunCount:0,
        ownedControllerConfigVerified:false, desiredProfileMatchesOwned:false},
      verification:{serviceOwnership:false, tunOwnership:false, routes:false, dns:false, internet:false}
    });
    case 'native-desktop-capabilities': return {schemaVersion:1,
      clipboardReadAvailable:true, clipboardWriteAvailable:true, filePicker:'zenity',
      configEditorAvailable:true, qrEncoderAvailable:true, gtk4FallbackAvailable:false};
    case 'native-core-readiness': return {schemaVersion:1, scope:'desktop_setup_facts', installed:true,
      version:'1.19.30', tunDevice:'present', fileNetworkCapabilities:'present', servicePermissionReadiness:'not_verified',
      coverage:{serviceContextVerified:false, tunCreationVerified:false, controllerQueried:false},
      remediation:'verify_native_host_setup'};
    case 'native-startup-capabilities': return envelope({runtimeOwnership:true, mutations:true, methods:['startup.configure']});
    default: return null;
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const value = process.argv.length === 3 ? response(process.argv[2]) : null;
  if (value === null) process.exitCode = 64;
  else process.stdout.write(JSON.stringify(value) + '\n');
}
