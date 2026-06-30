## ADDED Requirements

### Requirement: Enter shell command mode on `!`

When the user presses `!` in normal mode, the system SHALL enter shell command input mode, showing a `! ` prompt.

### Requirement: Type and edit shell command

In shell command mode, the user SHALL be able to type characters (appending to the command), press Backspace (removing last character), and press Esc (cancelling and returning to normal mode).

### Requirement: Expand `$1` to selected file path

When the user presses Enter, any occurrence of `$1` in the command SHALL be replaced with the absolute path of the file or directory under the cursor, quoted. If multiple items are selected, `$1` SHALL expand to the first selected path.

### Requirement: Expand `$@` to all selected paths

When the user presses Enter, any occurrence of `$@` in the command SHALL be replaced with space-separated absolute paths of all multi-selected items, each quoted. If nothing is multi-selected, `$@` SHALL expand to the path under the cursor (same as `$1`).

### Requirement: Execute command and show output

When the user presses Enter with a non-empty command, the system SHALL execute the command via `sh -c` in the current panel's directory, capture stdout and stderr, and display the output in QuickView using QuickViewMode::Text.

### Requirement: Command history via arrow up

In shell command mode, pressing `↑` SHALL cycle through previously executed commands (in-memory only, no persistence).

### Requirement: Empty command does nothing

Pressing Enter with an empty command SHALL be a no-op (stay in shell command mode).

#### Scenario: User runs `git log` on a file

- **WHEN** the user navigates to `src/main.rs`
- **AND** presses `!`
- **AND** types `git log $1`
- **AND** presses Enter
- **THEN** the system runs `sh -c "git log \"src/main.rs\""` in the current directory
- **AND** displays the git log output in QuickView text mode

#### Scenario: User runs command on selected directory

- **WHEN** the user navigates to a directory entry `my-project/`
- **AND** presses `!`
- **AND** types `du -sh $1`
- **THEN** the command expands to `du -sh "/absolute/path/to/my-project"`

#### Scenario: Multi-select expands `$@`

- **WHEN** the user has selected 3 files with Space
- **AND** types `ls -la $@`
- **THEN** the command expands to `ls -la "/path/a" "/path/b" "/path/c"`

#### Scenario: Arrow up recalls previous command

- **WHEN** the user runs `git status $1`
- **AND** presses `!` again
- **AND** presses `↑`
- **THEN** the input shows `git status $1`

#### Scenario: Cancel shell mode with Esc

- **WHEN** the user presses `!` and starts typing
- **AND** presses Esc
- **THEN** the shell mode exits and the user is back in normal mode with no command executed
