## ADDED Requirements

### Requirement: Recursive selected-size shown during multi-select
When one or more entries are multi-selected, the footer SHALL display the recursive size of the selection (files contribute their own size; selected directories contribute the recursive size of everything inside them) alongside the recursive total size of the current directory, in the form `"Selected: {selected size} of {directory total size}"`.

#### Scenario: Only files selected
- **WHEN** the user multi-selects one or more files (no directories) with `Space`
- **THEN** the footer shows `"Selected: {size} of {total}"` where `{size}` is the sum of the selected files' sizes

#### Scenario: A selected directory contributes its real recursive size
- **WHEN** the user multi-selects a directory
- **THEN** the selected size includes the recursive size of everything inside that directory, not zero and not just the directory entry's own metadata size

#### Scenario: Directory total reflects the whole current directory, not just the selection
- **WHEN** the user selects a subset of the entries in the current directory
- **THEN** the denominator (`{total}`) reflects the recursive size of every entry currently listed in the directory, independent of which entries are selected

### Requirement: Recursive size computation runs in the background
Recursive directory walks needed for either the selected size or the directory total SHALL run on a background thread so the UI remains responsive while they complete, showing a placeholder until both figures are ready.

#### Scenario: Selecting a large directory does not freeze the UI
- **WHEN** the user selects a directory containing a very large number of files
- **THEN** the terminal keeps accepting input and redrawing while the recursive size is computed in the background

#### Scenario: Placeholder shown while computing
- **WHEN** a selection changes and the recursive sizes are not yet ready
- **THEN** the footer shows the previous count-based `"Selected: N"` text until both the selected size and the directory total are available, then swaps to the `"Selected: {size} of {total}"` format

### Requirement: Directory total is cached per directory visit
The directory total SHALL be computed once per directory visit and reused across selection changes within that visit, recomputed only when the listing refreshes (navigation, filter change, or any operation that reloads directory entries).

#### Scenario: Toggling selection within the same directory reuses the cached total
- **WHEN** the user toggles selection on multiple entries in the same directory without navigating away
- **THEN** the directory total is computed once and reused for each subsequent selection change, not recomputed from scratch every time

#### Scenario: Navigating away invalidates the cached total
- **WHEN** the user navigates to a different directory
- **THEN** the previously cached directory total no longer applies and a fresh computation starts the next time a selection is made in the new directory

### Requirement: Superseded selection-size computations are cancelled
When the selection changes again before a previous recursive selection-size computation has finished, the stale computation's result SHALL be discarded and, where possible, the in-progress walk stopped early rather than continuing to completion unnecessarily.

#### Scenario: Rapid re-selection discards the stale result
- **WHEN** the user changes the selection again while a previous selection's recursive size is still being computed
- **THEN** the stale computation's eventual result is ignored and only the size for the current selection is shown once ready
