# Ubuntu pilot rollout and connection context

Manager image: `dwdesktop-manager:preview-20260910.10` in attic.
Agent package: `0.1.0-preview6`, verified HTTPS download and headless enrollment.

| Host | Device identity | Result |
| --- | --- | --- |
| Easternkingdoms | 16df48d0-3827-4dc0-963e-04a8f65335e5 | Existing integrated pilot preserved |
| Minigpu, 192.168.69.21 | fd6b6a59-40fc-41ac-84ef-b498ca8d6880 | Enrolled, online, full browser test passed |
| Spark, 192.168.69.28 | b9f6b90d-38f3-45fa-abc9-2297a827dffb | ARM64 agent running, online, full browser test passed |

The chooser and viewer show device name, reported hostname/platform, description,
and selected desktop environment/user. Breadcrumb navigation explicitly provides
Fleet console → Device → Desktop, without treating the brand logo as the only
way back. Device names are fetched by enrolled identity, not trusted URL labels.

`deploy/manager/enroll-pilot.py HOST` enrolls only the two allowlisted existing
pilots, sends codes through SSH stdin, installs the verified package, provisions
the restricted Unix socket directory, and updates/restarts only the managed
broker/viewer service. It checks existing desktop unit PIDs before/after. Private
keys remain on-device; enrollment codes are not printed. Previous broker source
is retained in the per-host private rollout temporary directory.

Browser tests:

```
node tests/integration/manager-session-bridge.mjs minigpu
node tests/integration/manager-session-bridge.mjs spark
```

Both passed explicit GNOME create, ready, reconnect, decoded video, keyboard and
mouse, reload reconnect, 1080p→4K→1080p, destroy only the test desktop, and return
via the breadcrumb to the fleet card. Both selected manager UDP30445 as the
media peer and reported zero JavaScript errors. Both input screenshots were
visually inspected and show the exact expected text in Text Editor. Evidence is
under `artifacts/manager-session-proof/{minigpu,spark}/`. Existing desktop units
and sessions remained active. No host, display-manager, or cluster restart.
Frontend suite: 73 tests passed; Docker TypeScript/Vite build passed.

Rocky office1/2/3 were not modified. Read-only inventory found attic Arcane,
Chaos and Shadow are Ubuntu, but none has the existing managed broker, Xorg or
GNOME session command. Their desktop-backend installation is a separate pending
operator choice because these are cluster control-plane servers; they were not
silently counted as rolled out or modified. No new desktop packages installed.

Remaining product work includes the agreed single confirmed Delete action
(revoke/disconnect/remove), explicit console-versus-managed-session presentation,
and the previously documented authentication/backend packaging limitations.
