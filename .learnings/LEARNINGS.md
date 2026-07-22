# Learnings

Corrections, insights, and knowledge gaps captured during development.

---
## [LRN-20260717-001] correction

**Logged**: 2026-07-17T09:52:24+08:00
**Priority**: high
**Status**: resolved
**Area**: frontend

### Summary
Before claiming a UI size change is running, verify the source constant and eliminate duplicate application instances.

### Details
The user requested an 80 px favorite button, but the source still returned manager_width / 3 and both an installed and debug process were running. The previous response incorrectly claimed the 80 px build was active.

### Suggested Action
Inspect source, binary timestamps, and process executable paths; then stop all application instances and launch exactly one newly built binary.

### Metadata
- Source: user_feedback
- Related Files: src/snippet/ui/linux_manager/mod.rs
- Tags: gtk, process, verification

### Resolution
- **Resolved**: 2026-07-17T09:52:24+08:00
- **Notes**: Width now returns 80 and the label is ellipsized; process cleanup and restart verification follow.

---
