# Fishez Terminal File Manager - Flowchart

Two layouts are available:
- Default single pane uses one `FilesView` and `TerminalUI::draw_ui`.
- Two-pane mode is a runtime toggle (`Ctrl+T`); `--two-pane` / `-2` remain startup aliases for the same state.

```mermaid
flowchart TD
    A["main.rs: main()"] --> B[Create mpsc channel]
    B --> C[Create TerminalUI]
    C --> D["Add Features (VsCode, Find, RipGrep, Favourites, Delete, Open, MultiSelect)"]
    D --> E[Create FilesView]
    E --> F["FilesView.update()"]
    F --> G["TerminalUI.draw_ui()"]
    G --> H[Event Loop]
    H -->|Key Event| I["handle_key_event()"]
    I --> J["TerminalUI.handle_features_shortcuts()"]
    J --> K["FeatureTrait.captured_key_event()"]
    H -->|Resize Event| L[Update TerminalUI columns/rows]
    L --> G
    H -->|Message Received| M[Update FilesView.files]
    M --> G

    subgraph "TerminalUI.draw_ui()"
        G1["draw_feature_header()"]
        G2["draw_default_header()"]
        G3["draw_system_header()"]
        G4["draw_feature_content()"]
        G5["draw_files_list() or draw_file_content()"]
        G6["draw_feature_footer()"]
        G7["draw_default_footer()"]
        G8["draw_footer_actions()"]
        G --> G1
        G1 -->|No feature header| G2
        G2 --> G3
        G3 --> G4
        G4 -->|No feature content| G5
        G5 --> G6
        G6 -->|No feature footer| G7
        G7 --> G8
    end

    subgraph Features
        D1[FeatureTrait]
        D2[drawn_header]
        D3[drawn_content]
        D4[drawn_footer]
        D5[modify_footer_actions]
        D6[map_item]
        D1 --> D2
        D1 --> D3
        D1 --> D4
        D1 --> D5
        D1 --> D6
    end
```

## Responsiveness verification

- In a large directory, type a filter and repeatedly backspace. Matching uses the last directory snapshot; clearing the filter restores its entries without rereading disk. Reenter the directory to pick up external changes.
- Submit a RipGrep search across a large tree, then move the cursor or change directories. Input should remain responsive, and results from the old directory must not replace the new listing.
- Preview text with F3, including syntax-highlighted files. A loading view appears while the worker runs. Press Escape before completion; the preview must stay closed.
- Confirm deletion of disposable files, then navigate while the trash operation runs. The originating directory refreshes on completion if it is still open; errors appear as notifications.
- Hold an arrow key while browsing a slow mount. Footer counts come from the listing, and free space refreshes in a worker at five-second intervals, with at most one disk query in flight per pane.
- Copy many small files. Progress updates are throttled to 50 ms, queued messages are processed in bounded batches, and each batch redraws once. Verify cancellation and conflict prompts still respond.

Directory enumeration on entry/refresh remains synchronous. These changes remove repeated enumeration while filtering; they do not guarantee a latency bound when entering a slow filesystem.
