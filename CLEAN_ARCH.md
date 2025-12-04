# Fishez Clean Architecture

This document describes the Clean Architecture structure implemented in Fishez.

## Directory Structure

```
src/
├── domain/                    # Inner Circle - Pure Entities
│   ├── mod.rs
│   └── entry.rs
├── application/               # Use Cases & Ports (Business Logic)
│   ├── mod.rs
│   ├── ports.rs
│   ├── state.rs
│   └── use_cases/
│       ├── mod.rs
│       ├── navigate.rs
│       └── file_ops.rs
├── infrastructure/            # Outer Circle - Implementations
│   ├── mod.rs
│   ├── fs_adapter.rs
│   ├── search_adapter.rs
│   ├── clipboard_adapter.rs
│   └── open_adapter.rs
├── presentation/              # UI Layer
│   ├── mod.rs
│   └── terminal/
│       ├── mod.rs
│       └── renderer.rs
├── logger.rs                  # Utility module for logging
├── lib.rs                     # Library root
└── main.rs                    # Composition Root & Event Handling
```

---

## Layer Breakdown

### 🔵 Domain Layer (`src/domain/`)
> **The innermost circle** - Contains pure business entities with no external dependencies.

| File | Structs/Enums | Description |
|------|---------------|-------------|
| `entry.rs` | `FileEntry` | Core entity representing a file/directory |
| `entry.rs` | `EntryKind` | Enum: `File` \| `Dir` |

**Dependency Rule:** Domain has NO dependencies on other layers.

---

### 🟢 Application Layer (`src/application/`)
> **Business logic & interfaces** - Defines what the app does and how it communicates with the outside world.

#### Ports (Interfaces)
| File | Trait | Description |
|------|-------|-------------|
| `ports.rs` | `FileSystemPort` | Interface for file system operations |
| `ports.rs` | `SearchPort` | Interface for search operations |
| `ports.rs` | `ClipboardPort` | Interface for clipboard operations |
| `ports.rs` | `OpenPort` | Interface for opening files |

#### State
| File | Structs/Enums | Description |
|------|---------------|-------------|
| `state.rs` | `AppState` | Global app state (panels, active pane, help) |
| `state.rs` | `PanelState` | Panel state (path, entries, cursor, scroll, mode, multi-selection) |
| `state.rs` | `PanelMode` | Enum: `Normal` \| `Filter` \| `QuickView(...)` |
| `state.rs` | `QuickViewMode` | Enum: `Text` \| `Image` \| `Directory` \| `NotSupported` |

#### Use Cases
| File | Functions | Description |
|------|-----------|-------------|
| `navigate.rs` | Navigation functions | Cursor movement, directory changes, search results |
| `file_ops.rs` | File operations | Delete, copy to clipboard |

**Note:** Multi-select and Favorites features are implemented in `main.rs` using the Clean Architecture ports and state.

**Dependency Rule:** Application depends only on Domain.

---

### 🟠 Infrastructure Layer (`src/infrastructure/`)
> **The outer circle** - Implements the ports using real libraries.

| File | Struct | Implements | Dependencies |
|------|--------|------------|--------------|
| `fs_adapter.rs` | `StdFileSystem` | `FileSystemPort` | `std::fs`, `trash` |
| `search_adapter.rs` | `FdSearchAdapter` | `SearchPort` | `fd` command |
| `search_adapter.rs` | `RipGrepAdapter` | `SearchPort` | `rg` command |
| `clipboard_adapter.rs` | `SystemClipboard` | `ClipboardPort` | `clipboard` crate |
| `open_adapter.rs` | `SystemOpenAdapter` | `OpenPort` | System commands |
| `open_adapter.rs` | `VsCodeAdapter` | `OpenPort` | `code` command |

---

### 🟣 Presentation Layer (`src/presentation/`)
> **The UI layer** - Pure rendering, no business logic.

| File | Struct | Description |
|------|--------|-------------|
| `renderer.rs` | `TerminalRenderer` | Draws TUI based on `AppState` |

**Key Principle:** The renderer is "dumb" - it only draws what it's given.

---

## Features

### Multi-select (Space)
- Toggle selection with `Space` key
- Multi-selected items highlighted in blue
- Bulk delete with `Ctrl+W`
- Clear selection with `Esc`

### Favorites (Ctrl+D)
- `Ctrl+D` opens favorites list
- `Ctrl+Shift+D` adds current directory to favorites
- Navigate with arrow keys, select with `Enter`
- Stored in `favorites.txt`

---

## Composition Root (`src/main.rs`)

```rust
// 1. Initialize Infrastructure (Adapters)
let fs_adapter = StdFileSystem::new();
let clipboard_adapter = SystemClipboard::new();

// 2. Initialize Application State
let mut app_state = AppState::new(two_pane_mode);

// 3. Initialize Presentation (Renderer)
let mut renderer = TerminalRenderer::new();

// 4. Event Loop - dispatch to Application Layer
loop {
    renderer.draw(&app_state);
    match event.code {
        KeyCode::Up => navigate::move_cursor(&mut panel, -1, rows),
        KeyCode::Enter => navigate::enter_selected(&fs, &mut panel),
        KeyCode::Char(' ') => panel.toggle_multi_selection(panel.cursor),
        // ...
    }
}
```

---

## Dependency Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     main.rs (Composition Root)              │
└─────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  Presentation   │  │  Infrastructure │  │   Application   │
│   (renderer)    │  │   (adapters)    │  │   (use cases)   │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              ▼
                    ┌─────────────────┐
                    │     Domain      │
                    │   (entities)    │
                    └─────────────────┘
```

**Key Rules:**
1. Dependencies point inward (toward Domain)
2. Domain knows nothing about other layers
3. Application defines interfaces (ports) that Infrastructure implements
4. Presentation only renders state - no business logic
