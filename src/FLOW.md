# Fishez Terminal File Manager - Flowchart

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