# Large-session staged loading design

## Goal

Make opening a large chat render its newest messages before any full-history-derived metadata, while keeping streaming detail complete and preserving layout geometry for the header total and left minimap.

## Root cause

Selecting a session currently starts message skeleton, user-nav, and context-window requests together. Each path reconstructs the full stitched transcript. Hermes API Server also ignores the caller's message `limit`, so a nominal 24-row request still transfers and parses the complete session.

## Architecture

### Phase 1: latest window

Add a Yahu-local latest-window path backed by read-only `state.db` queries. Resolve visible-history lineage using the existing session lineage rules, walk lineage segments newest-first, and read rows newest-first in bounded batches. Stop once enough raw rows exist to produce the requested skeleton window and the oldest retained turn has a user/system boundary. Reverse before the existing normalization and skeleton pipeline.

The initial response omits authoritative full-history metadata. It returns the latest compact window, `has_older`, `has_newer=false`, and a marker indicating that total/boundary metadata is pending. If local state is unavailable, use the existing full-history API path.

The active watch starts independently. An unfinished newest turn remains expanded as raw assistant/tool/reasoning rows; completed turns use existing `turn_details` metadata and lazy range hydration.

### Phase 2: metadata

After React commits the latest window for the selected session, start user-nav and context-window requests in parallel. User-nav supplies the authoritative stitched total and minimap data. Ignore responses whose session ID is no longer active.

The header keeps a fixed-width total element in the DOM while pending. `ChatUserNavigator` stays mounted and renders a fixed-width rail with inert placeholder bars until data arrives. Loaded data replaces children without changing the outer geometry.

## Data and race handling

- Track latest-window readiness by session ID, not a page-global boolean.
- Reset metadata values and loading flags when the active session changes.
- Abort or invalidate stale metadata requests on session change.
- Watch messages merge by stable identity with the latest skeleton window.
- Older/newer/around navigation retains existing cursor and scroll-anchor behavior.
- Failure of deferred metadata does not clear rendered messages.

## Error handling

- Local latest-window read failure falls back to the existing API reconstruction.
- User-nav failure leaves the minimap shell in an unavailable/empty state and preserves its dimensions.
- Context-window failure remains isolated from message rendering.

## Tests

- Backend latest mode reads a bounded tail and preserves chronological order.
- Tail collection crosses lineage segments and retains compression/reset visibility rules.
- Latest unfinished turn includes detail rows.
- Completed turns still expose lazy detail metadata.
- Frontend starts deferred metadata only after the latest-window commit.
- Header and minimap shells remain mounted while pending and after empty/error results.
- Stale metadata responses cannot update a newly selected session.
- Existing pagination, watch merge, detail hydration, and mobile layout tests remain green.
