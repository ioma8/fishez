## ADDED Requirements

### Requirement: F5 copies selected files to destination
Pressing F5 in normal or filter mode SHALL initiate a copy operation for the selected file(s).
In dual-pane mode the opposite pane's current path SHALL be pre-filled as the destination.
In single-pane mode an empty destination prompt SHALL appear.
The user SHALL be shown a confirmation dialog listing source item(s) and the resolved destination before the copy executes.
The system SHALL use `std::fs::copy` for files and recursive copying for directories.
After a successful copy the source pane SHALL refresh its listing; the destination pane (if visible) SHALL also refresh.

#### Scenario: Copy single file in dual-pane
- **WHEN** user presses F5 with one file selected and dual-pane is active
- **THEN** a confirmation prompt appears pre-filled with the opposite pane's path as destination

#### Scenario: User confirms copy
- **WHEN** user presses Enter on the confirmation prompt
- **THEN** the file is copied to the destination and both panes refresh

#### Scenario: User cancels copy
- **WHEN** user presses Esc on the confirmation prompt
- **THEN** no copy is performed and the pane returns to normal mode

#### Scenario: Copy with multi-selection
- **WHEN** user has toggled multiple files with Space and presses F5
- **THEN** the confirmation prompt lists all selected files and copies all of them to the destination

#### Scenario: Copy in single-pane mode
- **WHEN** user presses F5 in single-pane mode
- **THEN** an empty destination prompt appears for the user to type a path

#### Scenario: Copy fails due to IO error
- **WHEN** the copy operation encounters a filesystem error (permissions, disk full)
- **THEN** an error notification is shown in the panel and no partial state is left inconsistent
