# WP10 — Automatic physical-display / VKMS switching

Status: **queued, required console follow-up**, requested by Andrew 2026-09-06.
After the fixed-profile package rollout. No live display changes authorised
merely by this work item; Easternkingdoms must never be rebooted or have its
display manager restarted. Cluster-node reboots require separate cluster-aware
planning and approval.

## Desired behaviour

Move JetKVM from Easternkingdoms (Intel) to minigpu (AMD):

| Host event | Required destination |
| --- | --- |
| Easternkingdoms loses its last usable physical output | Existing console continues on VKMS |
| Minigpu gains a usable physical output | Existing console moves from VKMS to that physical output |
| Reverse the cable move | Both hosts reverse the transition |

KMS support itself does not disappear on unplug. The daemon must manage active
outputs and compositor adoption, not just select another `/dev/dri/cardN`.
“Connected” is not enough: destination mode, scanout and capture must be usable.
No alternate agent desktop/session, forced logout, or boot/BIOS access.

## Acceptance

- Keep the browser connection and existing desktop identity; retain the last
  frame with an honest transition status until a fresh destination keyframe.
- Release held input at topology changes, reject stale coordinates/queued
  events, validate new geometry, and require explicit control reacquisition.
  Preserve current lease safety: no replay of clicks or keys.
- Destination display is actually used by the greeter/current desktop. Verify
  a real picture and pointer alignment, not merely rising decode counters.
- Test both directions on Intel and AMD, at greeter and logged-in desktop,
  starting with 1080p. Repeat rapid unplug/replug and cold-start without a cable.
- Debounce hotplug and serialize transitions; distinguish monitor sleep,
  temporary mode-setting and last-output removal. Do not oscillate backends.
- Prepare and verify destination before retiring a usable source when possible.
  Bound recovery and expose failure; preserve/restore a working output rather
  than loop or claim success. Never silently disable a newly attached display.
- No driver-specific card-number assumptions. NVIDIA/Spark stays on its X11
  exception path; this item does not promise physical-KMS support there.

## Known implementation work

1. Inspect physical connector/EDID/active scanout events and compositor APIs;
   define physical-preferred/headless-fallback policy in daemon configuration.
2. Replace the fixed VKMS-only adoption helper. Today it calls
   `ApplyMonitorsConfig` with only `Virtual-1`, potentially disabling an attached
   HDMI output again. A guard pause alone is not a hotplug implementation.
3. Establish whether each target compositor supports live GPU/output adoption.
   Record an explicit unsupported case if it requires a desktop-manager restart;
   a restart is not transparent handoff acceptance.
4. Amend capture recovery contracts before accepting a different device/display
   identity or dimensions. Current recovery deliberately rejects such changes.
5. Add topology-bound input generation handling and real cable-move acceptance
   tests, retaining bounded queues, ownership and fail-closed behaviour.

Delegation when implementation starts: display policy/compositor integration;
capture/stream transition contracts; browser/input recovery and live acceptance.
Root owns shared contracts and coordinates physical cable moves with Andrew.
No GitHub issue created yet; this file is the local queued work item.
