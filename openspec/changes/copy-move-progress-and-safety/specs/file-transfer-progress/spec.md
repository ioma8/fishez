## ADDED Requirements

### Requirement: Background execution of copy and move
Copy and move operations SHALL run on a background thread so the terminal UI remains responsive and continues to redraw and accept input while a transfer is in progress.

#### Scenario: Large copy does not block input
- **WHEN** the user confirms a copy destination for a file or directory large enough to take more than a fraction of a second
- **THEN** the event loop keeps polling and redrawing (existing 100ms cadence) and the user can move the cursor or open menus while the copy runs in the background

#### Scenario: Small copy still completes and refreshes
- **WHEN** a copy or move of a small file completes on the background thread
- **THEN** both panel listings are refreshed and a completion notification is shown, matching current behavior for successful transfers

### Requirement: Progress reporting during transfer
While a copy or move is running, the system SHALL display an overlay showing the current file name, the count of items completed out of the total, and overall completion so the user can gauge how long the operation will take.

#### Scenario: Progress overlay shown for multi-file transfer
- **WHEN** the user copies or moves a multi-item selection
- **THEN** a progress overlay appears showing "N of M" items and the name of the file currently being transferred, updated as each item completes

#### Scenario: Progress overlay for single large file
- **WHEN** the user copies a single large file
- **THEN** the progress overlay shows the file name and updates periodically until the copy finishes

### Requirement: Cancellation of an in-progress transfer
The user SHALL be able to cancel a running copy or move operation. Cancellation stops before starting the next queued item; files already transferred remain at the destination (no automatic rollback).

#### Scenario: User cancels mid-transfer
- **WHEN** the user presses `Esc` while the progress overlay is shown
- **THEN** the background job finishes the file currently in flight, stops before starting the next one, and the UI reports how many items were transferred before cancellation

### Requirement: Per-file failure handling
Individual file failures (permission denied, disk full, source vanished, etc.) SHALL NOT abort the rest of a batch transfer. Each failure is recorded and a summary is shown when the transfer ends.

#### Scenario: One file fails, others succeed
- **WHEN** copying a multi-file selection and one file cannot be read (e.g., permission denied)
- **THEN** the remaining files are still copied, and the final notification reports counts of succeeded and failed items

#### Scenario: All files fail
- **WHEN** every item in the selection fails to transfer
- **THEN** the final notification clearly states 0 succeeded and the failure count, rather than reporting generic success
