# Design note: display model and login — console readback vs brokered virtual session

Companion to the 2026-09-06 architecture review. Records the decision about what surface M0 actually remotes, and how login relates to it. Prompted by a parallel implementer (Codex) hitting an XWayland blocker and evaluating a local user plus dummy display driver for the proof of concept.

## The decision at stake

There are two ways to obtain the pixels and input we remote. They prove different things and imply different login models. M0 must pick one deliberately.

- Model A, console readback: attach to the session already running on the physical console, read its framebuffer back, drive real hardware mode changes.
- Model B, brokered virtual session: the daemon owns a dedicated local user and starts a headless X session against a virtual (dummy) display driver, then captures and controls that.

## What forced the question: XWayland

On a Wayland desktop session, X11 APIs are not enough:

- X11 screen capture sees only XWayland (X11-compat) clients; native Wayland windows are invisible to it
- XTEST input injection does not reach native Wayland clients
- RandR through XWayland is per-client surface scaling, not a real output mode change — which is exactly the browser-scaling behaviour the acceptance doc forbids for the resize requirement

So capture, input and live resize against a real Wayland console are blocked, not merely awkward. This is the concrete form of the docs' unresolved "Wayland/headless strategy." Codex hit it directly.

## Model B sidesteps it entirely

A dedicated local user plus a dummy display driver does not use the Wayland session at all:

- start an Xorg server on xf86-video-dummy (or an equivalent headless setup) for a service user
- every application in that session is a native X11 client, so capture, XTEST and RandR all work natively with no XWayland gap
- resize is a real Xorg mode-set on the dummy driver: a genuine mode change that real X clients observe, not surface scaling, so it satisfies the resize contract honestly
- no physical monitor, no greeter on that session, no desktop-environment display daemon (mutter, kscreen) to fight over the mode

This is the landlord model at n=1 — the same virtual-display path the review flagged for M4, arriving early because it is the clean answer to the agent use case, not a workaround.

## What each model proves, and fails to prove

Model A (console readback) proves:

- real GPU framebuffer readback fidelity at native 4K
- RandR mode changes against real hardware driving a physical monitor
- remoting a session a human is also using locally

Model B (brokered virtual session) proves:

- the full protocol: capture, input, resize, terminal, auth — end to end
- resize as a real mode change (dummy mode-set), session and PTY preserved
- the agent session model and, at n=1, the landlord/bootstrap machinery

Model B does not prove the physical-console path: real-hardware 4K readback and real-hardware mode changes stay unverified. Given the agreed scope — agent use case first, single monitor, human-console deprioritised — that risk is acceptable and stays explicitly unretired. If a human-console product ever matters, Model A must be proven separately then.

## Recommendation

Adopt Model B for M0, endorsing Codex's direction. It is the only path that is not blocked on the likely Wayland pilot, it satisfies the resize contract honestly, it matches the agent-first and single-monitor scope decisions, and it front-loads the bootstrap machinery instead of building throwaway autologin.

Make the reclassification explicit so it is a decision, not a drift:

- M0 remotes a brokered virtual session, not the physical console
- the resize acceptance test runs against the dummy driver; note in the evidence that this is a virtual mode-set, and that physical-console resize is out of scope, not passed
- update review item 5: the "M1 requires an X11 session with RandR" pin becomes "M0 runs a dedicated Xorg-on-dummy session"; the RandR-versus-DE-display-daemon fight disappears because there is no desktop-environment display daemon in that session

## X11 deprecation risk (counterargument to Model B, and its mitigation)

Model B uses a headless Xorg plus xf86-video-dummy. Xorg is in maintenance mode, so this deserves a straight risk assessment.

- the X11 protocol is not the exposure; it will be spoken for years through XWayland
- the deprecated thing is the Xorg server, and distros are starting to drop the Xorg desktop session
- what we run is a headless Xorg plus dummy driver, the same shape as Xvfb and CI headless X, which has a large installed base and a long life; enterprise distros carry Xorg on 5-to-10-year support clocks
- the real liability: xf86-video-dummy is an Xorg DDX driver tied to the server's lifecycle, and we run it as a persistent daemon, so a future distro dropping or not patching the server leaves us on an unpatched dependency

Why this stays bounded:

- the display backend sits behind one seam, so dummy-Xorg to wlroots-headless is a backend swap, not a rearchitecture
- the M4 pod endgame is a wlroots headless compositor we own, which is Wayland-native and the long-term floor
- dummy-Xorg is therefore a scaffold: the fastest complete API surface to prove the protocol at M0, discarded when pods land

The actual danger is organizational, not technical: dummy-Xorg works, so the wlroots backend never gets written and the scaffold becomes production. Mitigation: name dummy-Xorg a scaffold in the milestone docs, and put the wlroots-headless backend on the M4 critical path as the real target, so X11 deprecation is an exit we schedule rather than a cliff we hit.

## How login changes under Model B

Model B removes the autologin crutch and replaces it with something closer to the endgame:

- the daemon owns a dedicated service user and starts that user's session itself; there is no human autologin, no greeter render, no seat fight for seat0
- this is the front half of the session-bootstrap milestone (PAM plus logind, start a session as a user) arriving at M0, with the display target set to the dummy driver
- the console-versus-brokered-login distinction from the review discussion resolves cleanly: M0 is already brokered-session, so the "no session exists" dead end does not apply — the daemon creates the session
- the later brokered-login feature (a credentials popup, then a Keycloak-to-OS identity mapping) is the same PAM machinery with the authentication source swapped; building it for a service user now means only the identity source changes later

## Bootstrap and the path to landlord mode (folds in the earlier virtual-driver discussion)

One component, built once, parameterised by display target and identity source:

- front half (authentication): service account now; credentials popup at the bootstrap milestone; Keycloak-to-OS identity assertion later. Same PAM authenticate plus open_session throughout — only the identity source swaps.
- back half (display target): dummy driver from M0; the same code targets a real console (Model A) or a pod's virtual display (M4) by changing one argument
- the Citrix and RDP virtual display driver lineage (Windows mirror driver to WDDM Indirect Display Driver) is the exact analogue: rendering into a virtual display removes the readback tax and makes resize a free mode-set with no physical output. xf86-video-dummy and headless Wayland compositors are the Linux equivalents.

Critical build rule for M0 (avoids foreclosing the wire-credential future): the daemon must create the session through PAM (pam_authenticate plus pam_open_session, registering with logind), even though M0's identity source is a trivial service account. Do not autologin or pre-start a fixed service-user session and merely attach to it — that rebuilds attach-to-existing, writes no PAM machinery, and turns the later wire-credential and Keycloak paths into fresh work rather than an identity-source swap.

Clarifying "not use a local user": there is no Linux session without a uid; PAM and logind always open a session as some user. What can be dropped is the pre-created fixed account. PAM can authenticate wire credentials against LDAP, SSSD or a Keycloak PAM module rather than /etc/passwd, and pods can provision an ephemeral user per session or map a Keycloak identity to a uid on demand. So "no dedicated local user" means an externally-backed or ephemeral uid, not the absence of a uid. The endgame (Keycloak device flow to token to identity assertion, PAM mapping to a uid) also means no raw password crosses the wire, retiring the credential-handling surface the review flagged.

Consequence for M0 structure: keep all session-facing code (capture, XTEST, RandR, later window events and AT-SPI) behind one internal seam that takes the display target as input, and keep session creation (PAM plus logind) as one component that takes the identity source as input. M2 (credentials popup) and M4 (pods, multi-session) then change arguments, not architecture.

## Coordination flag

Codex is making this display-model choice live while implementing. This note should be shared with that work immediately so M0 is built as a brokered virtual session by intent, with the review's contract amendments (screenshot message, CLI auth, per-request view operations) still applied. Confirm with Andrew before any change to a lab host: standing up an Xorg-on-dummy session and a dedicated service user is host configuration that the work packages do not themselves authorize.
