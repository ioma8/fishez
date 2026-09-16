# A Total Commander-style workflow on macOS and Linux

If your fingers remember F3 to view, F5 to copy, and Tab to switch panes, fishez brings that interaction style into a terminal. It combines those controls with typing to filter and a shell wrapper that changes directory when you exit.

## Install and start

With Homebrew:

```sh
brew install ioma8/tap/fishez
fishez --install-shell
```

Open a new terminal, then run `fz` from an existing project. The shell setup supports bash, zsh, and fish. See the [README](../README.md) for other installation methods.

## Find a file and inspect it

Start typing part of a filename. This filters the current directory; it does not search the whole project. Press `Esc` to clear the filter. Use arrows to select an entry, `Enter` to open a directory, and `Backspace` to go up.

Press `F3` or `Ctrl+P` to preview a file. Inside quick view, the left and right arrows switch files, and the up and down arrows scroll. Press `Esc` to close the preview.

For a file elsewhere in the project, use `Ctrl+F` for fd search. To search file contents, use `Ctrl+R` for ripgrep search. Homebrew installs both tools with fishez. `F4` or `Ctrl+O` opens the selected result in your editor, using `$EDITOR` with a VS Code fallback.

## Copy between two directories

Try this with disposable files first:

1. Press `Ctrl+T` to show two panes.
2. Navigate one pane to the source directory and the other to the destination, switching with `Tab`.
3. In the source pane, select a file, or use `Space` to select several.
4. Press `F5` or `Ctrl+Y` to copy to the opposite pane.

Transfers show progress in the background. Existing destination files trigger overwrite/skip choices; `Esc` cancels after the current file. `F6` moves, `Shift+F6` renames, and `F8` or `Ctrl+W` moves selected files to trash.

## Return to your shell

From normal browsing, press `Esc` to exit. If a filter or overlay is open, dismiss it first. Run `pwd` to see that your shell is now in the directory you were browsing.

This requires launching with `fz`. Running `fishez` directly opens the same file manager without changing the parent shell's directory.

## Things to know

- Release binaries cover macOS and Linux, on ARM64 and x86-64.
- Laptop function keys may require Fn; the Ctrl alternatives above are useful in that case.
- Text previews work without terminal image support. Inline image previews require kitty-graphics or iTerm2 image support.
- fishez is an independent project and is not affiliated with Total Commander.

If you try it, [report where the workflow got in your way](https://github.com/ioma8/fishez/issues/new), including your OS and terminal. The most useful feedback describes a real task you wanted to finish.
