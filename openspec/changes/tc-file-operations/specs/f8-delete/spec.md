## ADDED Requirements

### Requirement: F8 triggers the existing delete confirmation flow
Pressing F8 in normal or filter mode SHALL trigger the same delete confirmation flow currently activated by Ctrl+W.
Ctrl+W SHALL continue to work as an alias — no existing behaviour is removed.
The delete flow itself (confirmation dialog, trash/safe-delete, pane refresh) is unchanged; F8 is a new entry point only.

#### Scenario: F8 opens delete confirmation
- **WHEN** user presses F8 with a file selected
- **THEN** the same delete confirmation dialog appears as when pressing Ctrl+W

#### Scenario: Ctrl+W still works after F8 is added
- **WHEN** user presses Ctrl+W
- **THEN** the delete confirmation dialog appears (unchanged behaviour)

#### Scenario: F8 with multi-selection
- **WHEN** user has multiple files toggled with Space and presses F8
- **THEN** all selected files are listed in the delete confirmation dialog
