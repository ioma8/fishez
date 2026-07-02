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
│       ├── file_ops.rs
│       └── quick_view.rs
├── infrastructure/            # Outer Circle - Implementations
│   ├── mod.rs
│   ├── fs_adapter.rs
│   ├── search_adapter.rs
│   ├── clipboard_adapter.rs
│   ├── open_adapter.rs
│   └── favorites_adapter.rs
├── presentation/              # UI Layer
│   ├── mod.rs
│   ├── event_loop.rs
│   ├── input_handler.rs
│   ├── shortcuts.rs
│   └── terminal/
│       ├── mod.rs
│       ├── renderer.rs
│       └── overlays.rs
├── logger.rs                  # Utility module for logging
├── lib.rs                     # Library root
└── main.rs                    # Composition Root (< 100 LOC)
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
| `quick_view.rs` | Quick view operations | File preview, text/image/directory viewing |

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
| `favorites_adapter.rs` | Functions | N/A | `std::fs` |

---

### 🟣 Presentation Layer (`src/presentation/`)
> **The UI layer** - Event handling and rendering, no business logic.

| File | Struct/Functions | Description |
|------|------------------|-------------|
| `renderer.rs` | `TerminalRenderer` | Draws TUI based on `AppState` |
| `overlays.rs` | Overlay functions | Modal dialogs (delete, find, ripgrep, favorites) |
| `event_loop.rs` | `run()` | Main event loop handling |
| `input_handler.rs` | Handler functions | Keyboard event processing |
| `shortcuts.rs` | `handle()` | Feature shortcut handling |

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
- Stored in `~/.fishez/favorites.txt`

---

## Composition Root (`src/main.rs`)

The main.rs file is kept under 100 LOC and serves as the composition root:

```rust
// 1. Initialize Infrastructure (Adapters)
let fs = StdFileSystem::new();
let clipboard = SystemClipboard::new();

// 2. Initialize Application State
let mut state = AppState::new(two_pane);

// 3. Initialize Presentation (Renderer)
let mut renderer = TerminalRenderer::new();

// 4. Run Event Loop (delegated to presentation layer)
run(&mut state, &mut renderer, &fs, ...);
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
│   (renderer,    │  │   (adapters)    │  │   (use cases)   │
│   event_loop,   │  │                 │  │                 │
│   handlers)     │  │                 │  │                 │
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
5. main.rs is < 100 LOC and only wires layers together
