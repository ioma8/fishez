# fishez landing page implementation plan

## Context and outcome

Build a single public landing page at `https://ioma8.github.io/fishez/` that helps a developer understand fishez, see it working, and install it. The primary audience is macOS/Linux terminal users who like Total Commander-style controls. The primary action is installation; GitHub is the secondary action.

Reuse the existing [logo](Fishez_logo.svg) and [demo GIF](demo.gif) from the [README](README.md). At planning time they are approximately 9 KB and 467 KB, respectively. No new branding or demo recording is required for this first version.

This file is the implementation checklist. All tasks below are pending; creating this plan does not authorize starting implementation or deployment in this turn.

## Technical approach

Use plain HTML, CSS, and a small JavaScript file for clipboard and demo playback controls. No framework, npm dependencies, bundler, backend, analytics, cookies, or external font service. Essential content and installation commands must remain usable without JavaScript.

Keep source files under `site/`. A GitHub Actions workflow stages those files and the root README assets in `_site/`, then deploys only that directory through GitHub Pages. Copy the logo and GIF during staging rather than committing duplicates or fetching them from raw GitHub URLs at runtime. Add `_site/` to `.gitignore`.

Proposed files:

```text
site/
  index.html
  styles.css
  script.js
  assets/demo-poster.png       # First frame extracted from demo.gif
  social-preview.png          # 1200 × 630 image using existing logo and headline
  sitemap.xml
scripts/build-site.sh         # Stage site + shared assets; no dependency install
.github/workflows/pages.yml
```

The staged `assets/` directory also contains `Fishez_logo.svg` and `demo.gif`. Use relative local URLs such as `./styles.css` and `./assets/demo.gif` so the site works under the `/fishez/` project path. Use absolute production URLs only for canonical, sitemap, and social metadata.

## Page layout and copy

### 1. Header and hero

- Compact header: existing logo and fishez name; links to Install, Workflow, and GitHub. Keep navigation visible without a mobile menu component.
- H1: **Total Commander muscle memory, in your terminal.**
- Supporting line: **Type to filter. F3 to preview. F5 to copy. Leave your shell in the directory you found.**
- Short context: **A keyboard-driven file manager for macOS and Linux, with dual panes, familiar function keys, and actions for fd, ripgrep, and your editor.**
- Primary link: **Install fishez**, pointing to `#install`.
- Secondary link: **View on GitHub**, pointing to `https://github.com/ioma8/fishez`.
- Small factual line: **Open source · MIT licensed · Built with Rust**. Do not show a star counter or invent testimonials, adoption claims, or performance benchmarks.
- Place the existing GIF immediately below the hero, in a restrained terminal-style frame. Caption: **Filter a directory, preview files, search code, and return to your shell.**

### 2. Three workflow benefits

Use three short columns on desktop and stacked blocks on mobile:

1. **Find by typing.** Filter the current directory as you type. Use `Ctrl+F` for file search and `Ctrl+R` for content search.
2. **Familiar file operations.** `Ctrl+T` opens two panes, `Tab` switches panes, and `F5` copies to the opposite pane. Show `F3` for preview and `F4` for editor as keyboard labels.
3. **Continue in your shell.** Launch with `fz`, browse to a directory, and exit there. Explain that invoking `fishez` directly does not change the parent shell directory.

Link to the existing [workflow guide](docs/commander-workflow.md) on GitHub instead of duplicating a full documentation site.

### 3. Installation and first use

Give Homebrew the most prominent placement. Show this as a selectable code block with a Copy button:

```sh
brew install ioma8/tap/fishez
fishez --install-shell
```

Immediately below: **Open a new terminal, or source the file printed by setup. Then run:** followed by a separate `fz` block. Do not imply that `fz` is available in the existing shell immediately after setup.

Explain that the Homebrew formula includes fd and ripgrep, and bat is optional. Link to the README for Cargo and binary installation rather than maintaining multiple installation guides on the page.

Include a compact first-use sequence: type to filter → `Esc` clears the filter → `F3` / `Ctrl+P` previews → close overlays and exit from normal browsing → run `pwd`.

Copy buttons copy only their associated command block. Announce success through a polite live region. If clipboard access fails, leave the command selectable and display “Select and copy the command”; never claim success on failure.

### 4. FAQ and footer

Use native `<details>` elements for four questions:

- **Which platforms are supported?** Published binaries cover macOS and Linux, ARM64 and x86-64. Windows is not covered by the release/CI matrix.
- **Do I need a special terminal?** Text previews do not require image support. Inline image/GIF previews require kitty-graphics or iTerm2 image support.
- **What if my function keys control brightness or volume?** Mention Fn and the existing Ctrl alternatives: preview `Ctrl+P`, editor `Ctrl+O`, copy `Ctrl+Y`.
- **How is it different from other terminal file managers?** Describe the Commander-style defaults and typing-to-filter workflow, without claiming other tools lack previews or shell integration.

Footer links: GitHub, latest release, report an issue, workflow guide, and MIT license. Include a short independent-project statement: fishez is not affiliated with Total Commander. Invite feedback about a real task that felt difficult; keep starring the repository a secondary, optional request.

## Visual and accessibility requirements

- Use a dark terminal-inspired background, readable light text, and one accent color selected from the existing logo after inspecting it. Use system sans-serif text and system monospace for commands and key labels.
- Center content with a maximum width around 1100 px. Give the hero and demo most of the visual emphasis; avoid decorative dashboards, fake screenshots, and excessive cards.
- Use responsive type and spacing. Stack columns on narrow screens; contain horizontal scrolling inside command blocks, never across the whole page.
- Render the GIF at its natural aspect ratio with explicit dimensions and responsive width. Keep small-screen viewing possible by linking to the original asset.
- Start with a static poster and an explicit **Play demo** button. JavaScript swaps between the existing GIF and poster; **Stop demo** returns to the poster. Do not call this “pause,” because resuming restarts a GIF. No automatic animation, including for reduced-motion users. Without JavaScript, show the poster and a direct **Open animated demo** link.
- Provide descriptive alt text, semantic landmarks, one H1, logical headings, visible keyboard focus, a skip link, sufficient contrast, and adequately sized controls. Avoid using color alone to convey state.

## Ordered execution checklist

### 1. Verify inputs and prepare assets

- [ ] Read the current README and workflow guide; verify shortcuts, shell setup, and platform claims against current implementation before copying them.
- [ ] Inspect `Fishez_logo.svg` and `demo.gif`; confirm dimensions and select the accent color.
- [ ] Extract a static first frame from the existing GIF using an available image/video utility. Preserve the original GIF and logo.
- [ ] Create the social preview using the existing logo, headline, and a simple background; export a 1200 × 630 PNG and inspect it at thumbnail size.

### 2. Implement the static page

- [x] Add `site/index.html` with the sections and concrete copy above.
- [x] Add `site/styles.css` with responsive layout, readable commands, focus states, and reduced-motion handling for any CSS transitions.
- [x] Add `site/script.js` for copy feedback and opt-in demo playback. Hide JS-only controls until their handlers are ready.
- [x] Link the workflow guide and installation alternatives to their canonical GitHub locations.

### 3. Add discovery metadata

- [x] Set title to `fishez — Total Commander-style terminal file manager` and a concise description mentioning macOS/Linux, filtering, preview, and dual panes.
- [x] Add canonical URL `https://ioma8.github.io/fishez/`, Open Graph metadata, and a large-image social card referencing the absolute production PNG URL.
- [x] Reuse the SVG logo as the favicon and set the HTML language to English.
- [x] Add a sitemap containing the landing page URL; do not invent modification dates. A project-level robots.txt is unnecessary because crawlers use the host-root file.

### 4. Stage and validate locally

- [x] Add `scripts/build-site.sh` to stage only the explicit site files and shared assets into `_site/`. Resolve paths relative to the script/repository, not the caller's working directory. Avoid broad recursive deletion.
- [x] Ignore `_site/` and exclude `site/` plus `LANDING_PAGE.md` from Cargo packaging where needed, so website assets do not inflate crate releases. Check existing exclusions before editing `Cargo.toml`.
- [x] Document a local preview command in the README development section. Serve a parent directory with the staged content mounted as `fishez/`, so local verification exercises the actual project subpath.
- [ ] Validate HTML, internal anchors, asset references, and external destinations. Check the browser console for errors.
- [ ] Inspect at 375 px, 768 px, and 1440 px widths, plus 200% browser zoom. Confirm there is no page-wide horizontal overflow.
- [ ] Verify keyboard navigation, copy success/failure, playback/stop, reduced-motion preference, and the no-JavaScript experience.
- [x] Confirm the build artifact includes only the website, with no source tree, runtime logs, or developer files.
- [x] Run `git diff --check`; run `cargo check` and `cargo fmt -- --check` as required by repository guidance. No new Rust behavior tests are needed for this static page.

### 5. Configure GitHub Pages deployment

- [ ] Inspect existing Pages settings, custom domains, and workflows before making changes; preserve any existing site configuration unless it conflicts with this agreed destination.
- [x] Add `.github/workflows/pages.yml`, separate from the Rust release pipeline, with manual dispatch and deployment on pushes to `main` affecting `site/**`, shared logo/GIF, staging script, or the Pages workflow.
- [x] Run the staging script, then use the official configure, artifact-upload, and deploy Pages actions. Resolve supported action versions at implementation time using GitHub's documentation.
- [x] Grant `contents: read`, `pages: write`, and `id-token: write` only where needed. Use the `github-pages` environment, expose the deployment URL, and serialize Pages deployments with a concurrency group.
- [x] If build and deploy are separate jobs, make deploy depend on build; upload `_site/`, never the repository root. Do not deploy pull-request branches.
- [ ] Enable GitHub Pages with GitHub Actions as the publishing source and deploy the page.

### 6. Verify production and connect discovery paths

- [ ] Verify the deployed `/fishez/` URL returns successfully and CSS, JS, logo, GIF, poster, and social image all load from that subpath.
- [ ] Repeat installation-copy and demo-control checks over production HTTPS; check browser console and network failures.
- [ ] Inspect page-source metadata and load the social image URL directly. Do not assume social platforms refresh cached previews immediately.
- [ ] Set the repository homepage to the live URL and add a Website link near the README introduction after successful deployment.
- [ ] Report the live URL, changed files, verification results, and any remaining limitations. Mark this checklist complete only for work actually verified.

## Acceptance criteria

- [ ] A visitor can identify the product, watch the existing demo, and reach installation without opening GitHub first.
- [ ] The original logo and GIF remain the canonical assets used by both README and website.
- [ ] Installation instructions correctly separate shell setup from launching `fz` in a new shell.
- [ ] Essential content works without JavaScript; animations are opt-in and stoppable.
- [ ] Mobile layout and keyboard operation pass the checks above.
- [ ] The page is live over HTTPS at the agreed GitHub Pages URL with no broken assets or links.
- [ ] Subsequent changes to the site or shared assets automatically deploy from `main`.
- [ ] No runtime dependencies, accounts, telemetry, or backend services were introduced.

## Explicitly deferred

Custom domain, additional demo recordings, analytics, a blog engine, translations, email capture, social posting, and outreach. The first release is one polished page plus a link to the existing workflow guide. It does not claim that publishing the page alone will create traffic.

## References

- [README and installation instructions](README.md)
- [Commander workflow guide](docs/commander-workflow.md)
- [Shared demo](demo.gif) and [shared logo](Fishez_logo.svg)
- [GitHub Pages custom workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) — checked when preparing this plan; recheck action versions during implementation.
