## ADDED Requirements

### Requirement: F6 moves selected files to destination
Pressing F6 in normal or filter mode SHALL initiate a move operation for the selected file(s).
Destination logic SHALL be identical to file-copy: opposite pane pre-filled in dual-pane, empty prompt in single-pane.
The system SHALL attempt `std::fs::rename` first (same-filesystem fast path); if that fails with a cross-device error it SHALL fall back to copy-then-delete.
After a successful move the source pane SHALL refresh (items disappear) and the destination pane (if visible) SHALL refresh (items appear).

#### Scenario: Move single file in dual-pane
- **WHEN** user presses F6 with one file selected and dual-pane is active
- **THEN** a confirmation prompt appears pre-filled with the opposite pane's path as destination

#### Scenario: User confirms move
- **WHEN** user presses Enter on the confirmation prompt
- **THEN** the file is moved to the destination, removed from source, and both panes refresh

#### Scenario: User cancels move
- **WHEN** user presses Esc on the confirmation prompt
- **THEN** no move is performed and the pane returns to normal mode

#### Scenario: Cross-device move fallback
- **WHEN** source and destination are on different filesystems and rename fails
- **THEN** the system falls back to copy-then-delete and the result is identical to a same-device move

#### Scenario: Move with multi-selection
- **WHEN** user has toggled multiple files with Space and presses F6
- **THEN** all selected files are moved to the destination

#### Scenario: Move fails due to IO error
- **WHEN** the move operation encounters a filesystem error
- **THEN** an error notification is shown; if the cross-device fallback partially copied, the original files are not deleted
