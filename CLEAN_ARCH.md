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
└── main.rs                    # Composition Root
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
| `ports.rs` | `FileSystemPort` | Interface for file system operations (`list_dir`, `delete`, `read_file`, `is_file`, `is_dir`) |
| `ports.rs` | `SearchPort` | Interface for search operations (`find`) |
| `ports.rs` | `ClipboardPort` | Interface for clipboard operations (`copy`) |
| `ports.rs` | `OpenPort` | Interface for opening files with system apps (`open`) |

#### State
| File | Structs/Enums | Description |
|------|---------------|-------------|
| `state.rs` | `AppState` | Global application state (panels, active pane, help mode) |
| `state.rs` | `PanelState` | Single panel state (path, entries, cursor, scroll, mode, selections) |
| `state.rs` | `ActivePane` | Enum: `Left` \| `Right` |
| `state.rs` | `PanelMode` | Enum: `Normal` \| `Filter` \| `QuickView(...)` |
| `state.rs` | `QuickViewMode` | Enum: `Text` \| `Image` \| `Directory` \| `NotSupported` |

#### Use Cases
| File | Functions | Description |
|------|-----------|-------------|
| `navigate.rs` | `refresh_entries()` | Loads directory contents into panel |
| `navigate.rs` | `move_cursor()` | Moves cursor up/down |
| `navigate.rs` | `navigate_home()` / `navigate_end()` | Jump to first/last entry |
| `navigate.rs` | `enter_selected()` | Enter directory or open file |
| `navigate.rs` | `go_up_one_level()` | Navigate to parent directory |
| `navigate.rs` | `change_directory()` | Change to specific path |
| `navigate.rs` | `replace_entries_from_search()` | Populate panel with search results |
| `file_ops.rs` | `delete_selected()` | Delete files/directories to trash |
| `file_ops.rs` | `copy_to_clipboard()` | Copy path/name to clipboard |

**Dependency Rule:** Application depends only on Domain.

---

### 🟠 Infrastructure Layer (`src/infrastructure/`)
> **The outer circle** - Implements the ports using real libraries and system calls.

| File | Struct | Implements | Dependencies |
|------|--------|------------|--------------|
| `fs_adapter.rs` | `StdFileSystem` | `FileSystemPort` | `std::fs`, `trash` crate |
| `search_adapter.rs` | `FdSearchAdapter` | `SearchPort` | `fd` command (external) |
| `search_adapter.rs` | `RipGrepAdapter` | `SearchPort` | `rg` command (external) |
| `clipboard_adapter.rs` | `SystemClipboard` | `ClipboardPort` | `clipboard` crate |
| `open_adapter.rs` | `SystemOpenAdapter` | `OpenPort` | `open`/`xdg-open`/`start` commands |
| `open_adapter.rs` | `VsCodeAdapter` | `OpenPort` | `code` command |

**Dependency Rule:** Infrastructure depends on Application (ports) and Domain.

---

### 🟣 Presentation Layer (`src/presentation/`)
> **The UI layer** - Pure rendering, no business logic.

| File | Struct | Description |
|------|--------|-------------|
| `renderer.rs` | `TerminalRenderer` | Draws the TUI based on `AppState` |
| `renderer.rs` | `FooterActionsPosition` | Enum for footer layout |
| `renderer.rs` | `Message` | Enum for async UI messages |

**Key Principle:** The renderer is "dumb" - it only draws what it's given. All logic lives in the Application layer.

**Dependency Rule:** Presentation depends on Application (state) and Domain.

---

## Composition Root (`src/main.rs`)

The `main.rs` file wires everything together:

```rust
// 1. Initialize Infrastructure (Adapters)
let fs_adapter = StdFileSystem::new();
let clipboard_adapter = SystemClipboard::new();
let open_adapter = SystemOpenAdapter::new();

// 2. Initialize Application State
let mut app_state = AppState::new(two_pane_mode);

// 3. Initialize Presentation (Renderer)
let mut renderer = TerminalRenderer::new();

// 4. Event Loop
loop {
    renderer.draw(&app_state);
    
    match event.code {
        KeyCode::Up => navigate::move_cursor(&mut panel, -1, rows),
        KeyCode::Enter => navigate::enter_selected(&fs, &mut panel),
        // ...
    }
}
```

---

## Dependency Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     main.rs (Composition Root)              │
│  - Instantiates all layers                                  │
│  - Runs event loop                                          │
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
