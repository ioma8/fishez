## ADDED Requirements

### Requirement: Overwrite confirmation before replacing an existing file
Before a copy or move operation replaces an existing destination file, the system SHALL pause the transfer and prompt the user to choose: Overwrite, Skip, Overwrite All (remaining conflicts), Skip All (remaining conflicts), or Cancel the whole transfer. No existing file is overwritten without one of these explicit choices.

#### Scenario: Single file conflict
- **WHEN** copying a file whose name already exists at the destination
- **THEN** the transfer pauses and shows a conflict prompt naming the file before any bytes are written to the destination

#### Scenario: User chooses Skip
- **WHEN** the user selects Skip on a conflict prompt
- **THEN** that file is left untouched at the destination, the source file is not copied/removed, and the transfer continues with the next item

#### Scenario: User chooses Overwrite
- **WHEN** the user selects Overwrite on a conflict prompt
- **THEN** the destination file is replaced with the source file's contents

#### Scenario: User chooses Overwrite All
- **WHEN** the user selects Overwrite All during a multi-file transfer
- **THEN** the current and all subsequent conflicts in this transfer are resolved as Overwrite without further prompting

#### Scenario: User chooses Skip All
- **WHEN** the user selects Skip All during a multi-file transfer
- **THEN** the current and all subsequent conflicts in this transfer are resolved as Skip without further prompting

#### Scenario: User cancels on a conflict
- **WHEN** the user selects Cancel from the conflict prompt
- **THEN** the entire transfer stops immediately; files already transferred before the conflict remain, and no further items are processed

### Requirement: Directory merge instead of silent replace
When copying or moving a directory onto a destination path that already exists as a directory, the system SHALL merge into the existing directory (recurse and apply the same per-file conflict rules to files within) rather than silently replacing or duplicating its contents.

#### Scenario: Copying a directory that partially overlaps an existing one
- **WHEN** copying a directory whose destination already exists and contains some files with the same names and some different files
- **THEN** non-conflicting files are copied in alongside existing ones, and conflicting files trigger the same overwrite/skip prompt as single-file conflicts

#### Scenario: Moving a directory onto an existing directory
- **WHEN** moving a directory to a destination that already exists as a directory
- **THEN** the source directory's contents are merged into the destination using the same conflict rules, and the (now empty of unresolved items) source directory is removed only after all its contents have been resolved
