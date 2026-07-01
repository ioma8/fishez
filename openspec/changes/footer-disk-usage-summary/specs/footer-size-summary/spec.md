## ADDED Requirements

### Requirement: Disk free/total shown while browsing
When no multi-selection is active and the panel is showing a normal directory listing, the footer SHALL display the free and total space of the volume containing the current directory, right-aligned on the same row as the "N dirs, M files" count, in human-readable units, in the form "{free} of {total} free" (matching Total Commander's footer convention).

#### Scenario: Normal directory
- **WHEN** the active panel lists any directory on a mounted volume
- **THEN** the footer's counts row shows the dirs/files count on the left and the volume's free/total space, right-aligned, in human-readable units (e.g. "76.50 GB of 117.10 GB free")

#### Scenario: Disk usage cannot be determined
- **WHEN** the free/total space of the current directory's volume cannot be determined (unsupported platform, or the underlying syscall fails)
- **THEN** the right-aligned figure is omitted entirely, leaving only the existing left-hand dirs/files count

#### Scenario: Not affected by directory contents
- **WHEN** the active panel lists a mix of files and subdirectories, or an empty directory
- **THEN** the right-aligned figure reflects the volume's free/total space, independent of how many files or directories are listed (no per-entry size summation for the browsing case)

### Requirement: Selected-files size shown during multi-select
When one or more entries are multi-selected, the footer SHALL display the combined size of the selected files (not directories), right-aligned on the same row as the "Selected: N" text, instead of the whole-directory total.

#### Scenario: Multiple files selected
- **WHEN** the user multi-selects several files with `Space`
- **THEN** the footer's "Selected: N" row shows, right-aligned, the total size of just those selected files

#### Scenario: A selected directory is excluded from the size total
- **WHEN** the user multi-selects one or more directories along with files
- **THEN** the right-aligned total counts only the selected files' sizes, excluding directory entries (consistent with no recursive directory-size computation)

### Requirement: Right-aligned size is dropped when the terminal is too narrow
The size figure SHALL be omitted rather than overlapping or wrapping when the terminal is too narrow to fit both the existing left-hand text and the right-hand size on one row.

#### Scenario: Very narrow terminal
- **WHEN** the terminal is narrower than the combined width of the left-hand counts/selection text and the right-hand size string
- **THEN** the footer shows only the existing left-hand text, with no truncated or overlapping size fragment
