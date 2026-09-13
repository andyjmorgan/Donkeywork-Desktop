# Attic internal manager preview

Current image: `dwdesktop-manager:preview-20260912.5`. Adds a distinct existing
Wayland console entry for Minigpu and hides managed-only end/resize operations.
The portal adapter is deployed only on Minigpu; existing managed desktops remain.

Live URL: **https://192.168.10.11:30443**. No Keycloak, no public ingress,
no Cloudflare entry, no external DNS changes. Canonical intended hostname stays
desktops.donkeywork.dev; internal DNS and final TLS routing are not configured.

The browser will warn until its trust store accepts the dedicated preview CA.
Automation verified HTTPS using that CA and pinned the exact server public key
in Chromium; it did not globally disable certificate checks. The installer must
likewise have explicit CA trust, not use an insecure TLS flag.

Public preview CA fingerprint (SHA256):
`1E:C9:02:A1:41:C6:2A:47:5E:23:DA:C8:A0:C1:9B:70:1C:B0:FE:58:C2:CB:0D:09:8D:2C:CD:58:3B:75:DF:77`.
Local private bootstrap material is retained outside the repository under
`/home/localuser/.local/state/dwdesktop-manager-preview/pki` (0700).
Cluster Secret `donkeywork-desktop/manager-secrets` is the live source of truth;
its database password was rotated after bootstrap. Do not reuse the initial
password file from the local bootstrap directory. No private keys in Git/images.

## Deployment

Source: attic.yaml. Dedicated namespace donkeywork-desktop; Go manager pinned
to chaos for this locally imported preview image, HTTPS NodePort 30443 with
externalTrafficPolicy Local. Image: dwdesktop-manager:preview-20260910.10.
Build with `docker build -f manager/Dockerfile -t dwdesktop-manager:preview-20260910.10 .`
from the repository root, import to chaos containerd, then apply attic.yaml.
Future builds must use a new version tag. Not an HA/registry rollout.

PostgreSQL 17 uses a dedicated 5Gi nfs-client PVC. Manager and database are
single-replica. Manager ingress allows private lab 192.168.0.0/16; database
ingress allows only manager pods. No service account tokens. Containers run
non-root with dropped capabilities. The manager has read-only root filesystem.
Database traffic is private in-cluster TCP, not TLS in this preview.

Do not delete the PVC to restart: the attic NFS provisioner can delete its
underlying data on PVC deletion. No backup policy has been configured yet.

## Acceptance, 2026-09-10

Preview .2 adds permanent Delete for revoked records only, with confirmation.
The API enforces the revocation prerequisite atomically. Deletion removes the
registration/certificate metadata, not software on the host; it cannot be undone
from the UI. Missing/deleted identities must remain denied by future device auth.

Real browser test `tests/integration/manager-registration.mjs` passed creation,
one-time code display, reload hiding the code, regeneration invalidating old
code, CSR claim issuing a signed device certificate, consumed-code rejection,
revocation and zero browser JS errors. Screenshot visually reviewed at
artifacts/manager-proof/attic-manager.png. Codes are not printed or captured.
Two revoked verification records remain as explicitly labelled test history.

PostgreSQL pod was replaced via a scoped deployment restart; both enrollment
records, certificate metadata and revocation state survived on the PVC.
All three attic nodes remained Ready. No host, cluster, physical desktop or
existing pilot service was restarted. Frontend regression tests: 73 passed.

Preview .4 adds a separate mTLS device listener on LAN NodePort 30444, serving
/connect. Manager API/UI remain on 30443 without client-certificate prompts.
Installer archive is available under /downloads; manager-preview-trust ConfigMap
mounts the public preview CA at /downloads/manager-ca.crt. Provision that ConfigMap
from the public CA (not its key) before applying the updated Deployment.

Live online/offline and immediate revocation are wired. The package installs a
dedicated unprivileged outbound service; Easternkingdoms is enrolled. The existing
desktop processes are preserved.

Preview .7 adds managed-session RPC and H.264/input relay, validated first on
Easternkingdoms. Browser UDP30445 terminates at the manager and is forwarded over
the existing outbound device connection. Its NodePort equals the bound UDP port.
Only the dedicated manager Service/Deployment/NetworkPolicy changed in attic.
Certificate renewal, multi-user authentication and fleet backend provisioning
remain outstanding. See the session-bridge validation note for actual evidence.

Preview .9 moves Open desktops onto each available device card. Online is a
prominent green badge, backed by live device presence (heartbeat every15s,
45s liveness window, UI inventory refresh every5s). Offline/revoked are red;
failure to fetch current inventory shows Unknown instead of stale green Online.
Desktop navigation is disabled when offline, unknown, or no broker is advertised.
