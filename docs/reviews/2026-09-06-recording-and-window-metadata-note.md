# Future-design note: session recording and window/activity metadata

Companion to the 2026-09-06 architecture review. Design seed for a future recording milestone; nothing here is authorized implementation. Informed by Andrew's Horizon session-recording experience — the recording lifecycle and management plane (session event delegates, recording management), not the encoder/mux backend.

## Sequencing and isolation (Andrew, 2026-09-06 — firm)

Deferred. Recording and the session-bootstrap milestone are both out of scope until the protocol is proven (M0) and the web UI is proven (M1b). Neither may add contract fields or shape design decisions before then.

One architectural invariant is load-bearing now, even though the feature is deferred: recording is isolated from the capture layer.

- the worker's capture layer produces encoded frames plus damage events and has no knowledge that recording exists
- recording is a consumer at the broker media fan-out, reading the same opaque bytes a browser receives
- test: deleting the entire recording component leaves capture and streaming byte-for-byte unchanged
- recording never requests a keyframe from the encoder directly; it uses the same consumer-side keyframe-request path as any viewer, so its cadence cannot distort the live stream

This is why the M1 media path must stay multi-consumer capable. Do not build a single-reader pipe. This is the only recording-driven constraint M1 must honour; everything below waits.

## What recording is, mechanically

A tap at the broker that remuxes the live media stream to fragmented MP4. It is a remux, not a re-encode:

- the framed media adapter already carries complete access units with session-relative timestamps
- stream.configure's declared decoder bytes are exactly the MP4 codec configuration box (for H.264, SPS/PPS become avcC)
- v0 forbids B-frames, so decode order equals presentation order and the muxer needs no reordering logic
- H.264 is the near-certain M1 codec (only universal 4K-capable browser decode plus universal hardware encoders), but the transport is codec-agnostic, so the recorder must read codec/profile from stream.configure, not assume H.264

## Resolution changes are segment boundaries, not new recordings

A resolution change is one session's streamGeneration increment, which already forces a fresh keyframe.

- the recorder writes a new fMP4 init segment with the new dimensions and keeps appending to the same logical recording
- this is how DASH and HLS handle rendition switches; one session equals one recording with generation-boundary markers
- plain (non-fragmented) MP4 cannot change dimensions mid-file and would force separate files, so fMP4 is required, not optional
- the 4K to 1080p to 4K acceptance sequence produces one recording with two generation boundaries

## Keyframes: consumer-driven, activity-gated

Seekable recording requires periodic keyframes. Low-latency live streaming wants almost none (a 4K keyframe is megabytes), so the encoder runs an effectively infinite GOP and emits keyframes only at stream start, generation change and decoder recovery. These two needs are reconciled on the consumer side:

- while a recording is active, the recorder issues the existing keyframe request at its chosen cadence; when no recording runs, the live path stays lean at no cost
- cadence is gated on activity: request a keyframe after N seconds of emitted frames since the last one, not N seconds of wall-clock, so idle spans cost nothing
- the first frame after a long idle gap is requested as a keyframe even though a delta would decode; it is nearly free and turns each activity boundary into a clean seek point
- the spec promises "fragments begin at keyframes, best-effort N-second cadence," never a fixed GOP, because cadence is a consumer request and not an encoder guarantee

Cadence choice trades seek granularity against recording bitrate at 4K:

- roughly 2 to 5 seconds gives player-grade scrubbing but noticeable bitrate spikes at 4K
- roughly 30 to 60 seconds is cheap; a seek lands within that window then decodes forward, which suits agent-session review footage

## Seek cost is bounded by cadence, not recording length

A seek to any point jumps to the nearest preceding keyframe (fMP4 fragment offsets are indexed, so this is a table lookup, not a scan) and decodes forward. Worst-case forward decode is N seconds regardless of whether the recording is four minutes or four hours.

For desktop content the real cost is smaller than N suggests: capture is damage-driven, so an idle or near-static screen emits few frames, and inter-frames of a static desktop are near-empty. Full-motion video playing inside the recorded desktop is the pathological case. If instant scrubbing anywhere is required, generate a thumbnail or scrub track from the finished recording in post-processing; that adds no live-path cost and is what session-replay products do.

## Idle behaves correctly by construction

Capture is damage-driven (XDamage on X11), so an idle desktop emits no frames and the recording naturally costs bitrate proportional to activity, not to session length. A four-hour session busy for twenty minutes produces roughly twenty minutes of video in both bandwidth and file size; seeking into an idle span shows the last frame before it, instantly.

One muxing caveat: a muxer only learns a sample's duration when the next sample arrives, and some players scrub badly across hour-long sample durations. The standard screencast fix is a tiny repeat or skip frame (a few bytes) every few seconds while idle, keeping player timelines honest at near-zero bitrate. Optional; decide in the recording spec.

Late joiners (a viewer or recorder attaching during idle) would otherwise see nothing until the next change; the existing on-demand keyframe request on attach covers this.

## Metadata sidecar: idle and active-window tracking

Horizon lesson (Andrew): the features always wanted were idle-time detection and knowing which top-level window the user was interacting with at each moment, tracked via chapter and subtitle metadata beside the MP4. Both are cheap here because the events already exist or are planned:

- activity and idle chapters: derived from frame-emission gaps; a chapter per activity burst, idle spans labelled and skippable
- active-window timeline: event-driven on X11 through PropertyNotify on _NET_ACTIVE_WINDOW plus _NET_WM_NAME and WM_CLASS changes, giving (timestamp, windowId, app, title) with no polling; this rides the same EWMH window plane sketched for agent window-enumeration, so one event source serves both consumers
- control attribution: control-lease acquire, release and takeover events record who was driving (human or agent) at every moment, directly serving M3 human-takeover review; this is new versus Horizon and falls out of lease events we already have
- structural markers: display.resize.result (generation boundaries), terminal.opened and terminal.exit, attachment and reconnect events

Format:

- one JSON event-log sidecar is the source of truth: timestamped events on the same session-relative clock as captureTimeUs
- WebVTT chapter and caption tracks are generated from it for player UX, so scrubbing shows the active window and who was driving
- embedded MP4 chapter atoms are optional, for third-party players
- sidecar and video share sessionId and sessionEpoch and the generation markers, so they cannot be mispaired

## Privacy and policy

- recording needs a session.record permission, an explicit consent surface and retention rules
- a recorded desktop captures everything on screen; window titles leak document names and URLs
- the sidecar falls under the same consent and retention policy as the video
- recording and its metadata inherit the no-secrets-in-logs rules: no grants, keystrokes, clipboard or window titles in diagnostic logs
- audio does not exist in the contract; recordings are video-only until an audio amendment lands

## Contract asks (when this activates)

- session.record permission and recording lifecycle events (started, stopped, failed, rotated)
- recorder registered as a media consumer with keyframe-request rights but no input rights
- window-plane events (active window changed, title changed) as subscribable control-plane notifications, shared with the agent window-enumeration feature
- an explicit statement that all metadata events carry session-relative timestamps on the same clock as captureTimeUs

## Design constraints to preserve in M1

- keep the media path multi-consumer capable (broker fan-out); do not design a single-reader pipe
- keep screenshot and capture per-display and frame-addressable, so a still, a stream and a recording are the same source
- keep event notifications (topology, lease, terminal, window) on the control plane, where a non-video consumer can subscribe without touching media
