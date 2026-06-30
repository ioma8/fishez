## ADDED Requirements

### Requirement: F7 creates a new folder in the current directory
Pressing F7 in normal or filter mode SHALL open a new-folder prompt with an empty input.
Pressing Enter with a non-empty name SHALL create the directory via `std::fs::create_dir` inside the active pane's current path.
Pressing Esc SHALL cancel without creating anything.
After a successful creation the pane SHALL refresh and the cursor SHALL move to the newly created folder.
If a file or directory with the given name already exists, the system SHALL show an error notification and leave the prompt open.
If the name is empty, Enter SHALL be a no-op.

#### Scenario: F7 opens empty name prompt
- **WHEN** user presses F7 in normal mode
- **THEN** a new-folder prompt appears with an empty input field

#### Scenario: User types name and confirms
- **WHEN** user types a folder name and presses Enter
- **THEN** the directory is created and the pane refreshes with the cursor on the new folder

#### Scenario: User cancels
- **WHEN** user presses Esc in the new-folder prompt
- **THEN** no directory is created and the pane returns to normal mode

#### Scenario: Name already exists shows error
- **WHEN** user types a name that already exists in the current directory and presses Enter
- **THEN** an error notification is shown and the prompt stays open

#### Scenario: Empty name is rejected
- **WHEN** user presses Enter without typing anything
- **THEN** no directory is created (no-op)
