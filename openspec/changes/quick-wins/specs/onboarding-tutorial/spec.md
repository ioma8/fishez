## ADDED Requirements

### Requirement: Show onboarding banner on startup

When fishez starts, it SHALL display a compact banner in the header area showing the essential keyboard shortcuts. The banner SHALL be non-modal — the file list renders underneath and typing still works.

### Requirement: Auto-dismiss after timeout

The onboarding banner SHALL automatically disappear after 2 seconds from startup.

### Requirement: Dismiss on any keypress

Any keypress SHALL immediately dismiss the onboarding banner.

### Requirement: No persistent state

The onboarding SHALL use no config files or persisted state. It shows every startup but is dismissed so quickly by experienced users that it's effectively invisible.

#### Scenario: New user sees shortcuts

- **WHEN** fishez starts for the first time
- **THEN** a banner showing essential shortcuts (↑↓ navigate, Enter open, F3 preview, F4 VS Code, F6 find, F7 rg) is rendered in the header area

#### Scenario: Banner auto-dismisses after 2 seconds

- **WHEN** the onboarding banner is shown
- **AND** the user does not press any key
- **THEN** after 2 seconds, the banner disappears and the normal header is shown

#### Scenario: Any keypress dismisses banner

- **WHEN** the onboarding banner is shown
- **AND** the user presses any key
- **THEN** the banner disappears immediately

#### Scenario: Banner does not block interaction

- **WHEN** the onboarding banner is shown
- **AND** the user starts typing a filter
- **THEN** the filter works normally and the banner disappears
