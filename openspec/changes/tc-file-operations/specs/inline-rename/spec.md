## ADDED Requirements

### Requirement: Shift+F6 opens an inline rename prompt
Pressing Shift+F6 in normal or filter mode SHALL open a rename prompt pre-filled with the current item's name (filename only, not full path).
The prompt SHALL support standard text editing: character input, Backspace, and a cursor at end of input.
Pressing Enter SHALL commit the rename via `std::fs::rename` within the same directory.
Pressing Esc SHALL cancel without making any changes.
After a successful rename the pane SHALL refresh and the cursor SHALL remain on the renamed item.
If the new name is empty or identical to the current name, Enter SHALL be a no-op (no filesystem call made).
If a file with the new name already exists, the system SHALL show an error notification and leave the prompt open.

#### Scenario: Shift+F6 opens prompt with current name
- **WHEN** user presses Shift+F6 with a file under the cursor
- **THEN** a rename prompt appears pre-filled with the file's current name

#### Scenario: User edits and confirms rename
- **WHEN** user edits the name and presses Enter
- **THEN** the file is renamed on disk and the panel refreshes with the cursor on the renamed item

#### Scenario: User cancels rename
- **WHEN** user presses Esc in the rename prompt
- **THEN** no rename occurs and the panel returns to normal mode

#### Scenario: Rename to existing name shows error
- **WHEN** user types a name that already exists in the directory and presses Enter
- **THEN** an error notification is shown and the prompt stays open

#### Scenario: Empty name is rejected
- **WHEN** user clears the prompt and presses Enter
- **THEN** no rename occurs (treated as cancel or silent no-op)
