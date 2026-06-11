# Browser MCP & Advanced Browser Guidelines

## Browser Panel WebView Z-Order Guidelines
- **Never use native-popup menus (menu_button) over the WebView content area.** Native WebView2 renders as a child window above egui's immediate-mode rendering. Popups like `ui.menu_button` that extend over the WebView area will appear BEHIND the WebView and be unusable.
- **Use inline dual buttons instead of menus for WebView toolbar actions.** For screenshot and similar toolbar actions that need multiple options, render side-by-side buttons within a single bordered frame directly in the toolbar (e.g., `[ Full page | Visible area ]`). This avoids all menu/popup complexity and keeps controls in the egui layer above WebView.
- **WebView must yield during MODAL overlay interactions only.** When actual modal overlays (dropdown menus, context menus, popups) are active in the browser panel, hide the WebView via `SetIsVisible(false)` so the overlay appears above it. Simple hover tooltips on toolbar buttons/tabs do NOT trigger WebView hiding—these render safely in the egui layer above WebView without needing native hide/show cycles.
- **Toolbar hover does NOT trigger WebView hide.** The toolbar buttons and tab strip render in egui's paint layer above the native WebView. Hover tooltips on these controls remain visible without requiring `browser_panel_overlay_active` to be set or WebView to be hidden. This prevents the black/white flicker bug where hovering toolbar caused WebView content to disappear.
- **Toolbar and tab strip tooltips must appear centered above buttons.** Standard egui tooltips appear below widgets by default, which causes them to overlap the WebView content area and potentially get obscured. Use `browser_toolbar_icon_button()`, `browser_toolbar_toggle_button()`, or `show_tooltip_above()` helpers to position tooltips centered above toolbar buttons. These helpers use `egui::Area` with `CENTER_BOTTOM` pivot so the tooltip is horizontally centered on the button, not offset to the side. The gap is controlled by `BROWSER_TOOLBAR_TOOLTIP_GAP` constant.
- **Tab strip tooltips must also appear centered above.** Tab close buttons, tab titles/URLs, and the add tab (+) button must use `show_tooltip_above()` instead of standard `on_hover_text()` to ensure tooltips render centered above the tab strip and avoid WebView overlap.
- **Use grace period for smooth modal overlay transitions.** When modal overlay closes, keep WebView hidden briefly (150ms via `BROWSER_OVERLAY_GRACE_PERIOD_MS`) to prevent flickering when moving mouse between controls.
- **No per-project menu state needed.** With inline buttons instead of togglable menus, no runtime state tracking (like `browser_screenshot_menu_open_by_project`) is required—buttons are always visible and clickable.

## Browser Panel Performance Guidelines
- **Cache native WebView2 state to avoid redundant COM calls.** The `EmbeddedBrowser` struct maintains `cached_visible: Option<bool>` and `cached_bounds: Option<BrowserBounds>` to track the last applied native state. `set_visible_internal()` and `sync_position_internal()` check these caches before calling WebView2's `SetIsVisible()` or `SetBounds()`.
- **Update native state caches only after successful native calls.** Never write `cached_visible` or `cached_bounds` after a failed `SetIsVisible()`, `SetBounds()`, or host-window move. Leaving the cache unchanged ensures the next frame retries instead of permanently believing stale native state was applied.
- **Sync bounds before showing the browser.** In `sync_embedded_browser()`, always call `browser.sync_position(&bounds)` before `browser.show()`. This prevents the browser from becoming visible at wrong/old dimensions, which can cause white flicker.
- **Child-host show order prevents white rectangles.** When showing a browser, first move/resize the child host, then set WebView2 bounds, then call `SetIsVisible(true)`, then show the host HWND. The host window class must avoid background erasing so it cannot paint a white rectangle during WebView2 repaint delays.
- **Reset cached state on shutdown.** The `shutdown()` method must clear `cached_visible` and `cached_bounds` to ensure clean state when the browser is recreated.
- **Idempotent native operations prevent scroll flicker.** During scroll operations, egui may re-layout the browser panel every frame. Without caching, repeated `SetBounds()` calls to WebView2 cause the child window to invalidate and repaint, producing white/blank flicker artifacts.
- **GPU fallback is opt-in only.** `MERGEN_WEBVIEW2_DISABLE_GPU=1` may add WebView2 browser arguments such as `--disable-gpu` for driver-specific blank/white rendering issues. Keep it disabled by default because GPU composition is normally faster and more compatible.

## Browser Panel Compact UI Guidelines
- **Browser panel UI must minimize vertical chrome to maximize WebView space.** The panel should allocate ~60-80px total for all UI chrome (tabs + toolbar), leaving the rest for the embedded browser content.
- **Avoid separate header rows for titles or project names.** The browser panel header should not have a dedicated "Browser" title row or separate project name display; use the activity rail and terminal context to indicate the active project.
- **Tabs must stay on a single row using horizontal scroll.** Use `ScrollArea::horizontal()` around the tab strip to prevent tabs from wrapping to multiple lines. This keeps tab height predictable (22px) regardless of panel width.
- **Place add tab button inside ScrollArea next to last tab.** The add tab (+) button should be rendered inside the `ScrollArea::horizontal()` block, immediately after the tabs loop. This ensures the button stays visually connected to the last tab and scrolls with the tab strip. Button width should be ~28px with 14px icon.
- **Combine URL input and action buttons into one compact toolbar row.** Place the URL input field on the left taking available width, followed by icon-only buttons (Go, Clear, Design Inspect, Screenshot) on the same row with minimal 4px spacing.
- **Use reduced padding and margins throughout.** Inner margins should be 6px (not 10px); spacing between UI sections should be 4-6px (not 8-16px).
- **Reduce tab dimensions for compactness.** Tab height should be 22px (not 26px); tab close button should be 16px (not 18px); tab font should be 11px (not 12px).
- **Preserve all functionality in compact layout.** URL editing with double-click to select all, Enter to submit, tab switching/closing, and all toolbar buttons must remain fully functional. Do not add custom right-click context menus to URL input; rely on standard keyboard shortcuts and native egui behavior.
- **Maintain minimum URL input width.** The URL field should have a minimum width of 100px to remain usable even at narrow panel widths.
- **Use scrollable tabs at max tab limit.** With 5 tabs (BROWSER_MAX_TABS_PER_PROJECT), horizontal scrolling must work smoothly without clipping tab content.

## Browser Tab Lifecycle Guidelines
- **Closing the last tab leaves the browser empty (no auto-recreate).** When the last browser tab is closed via the X button or MCP `close` action, the browser enters an empty state with zero tabs. The tab state maps are cleaned up (`active_browser_tab_by_scope`, `browser_tabs_by_scope`, `browser_url_draft_by_scope` removed for the scope), and any active/inactive WebViews are shut down. Do not automatically recreate a new empty tab.
- **The (+) Add Tab button creates the first tab when none exist.** When the browser panel is in an empty state (no tabs), clicking the "+" button in the tab strip creates the first tab. If `browser_last_url` has a saved URL, the first tab is created with that URL pre-filled and navigation is triggered automatically.
- **Opening browser panel with saved URL auto-creates first tab.** When `draw_browser_panel()` detects that the browser is opening and `browser_last_url` exists but no tabs exist, it automatically creates the first tab with the saved URL and triggers navigation. The user does not need to press Enter or click Go.
- **URL input is empty when no tabs exist.** When the browser panel has no tabs, the URL input field should be empty (not auto-filled with `browser_last_url`). This ensures a clean state for the empty browser. The draft is only populated with `browser_last_url` when at least one tab exists.
- **Explicit tab creation only.** Do not call `ensure_browser_tab_state()` from `draw_browser_panel()`, `add_browser_tab()`, or `close_browser_tab()` to auto-create tabs. Tabs should only be created explicitly via:
  - User clicking the "+" button (`add_browser_tab` with `None` URL)
  - Auto-creation on panel open with saved URL
  - MCP `new` action
  - Video recording completion opening a recording tab
- **Cleanup on last tab close.** When `close_browser_tab()` closes the last remaining tab, it must:
  1. Remove `active_browser_tab_by_scope` entry for the scope
  2. Remove `browser_tabs_by_scope` entry for the scope
  3. Remove `browser_url_draft_by_scope` entry for the scope
  4. Remove all inactive browsers for the scope from `inactive_browser_tab_browsers`
  5. Shut down the active WebView if the closed tab was active

## Browser MCP Single-Binary Guidelines
- **Browser MCP helper runs inside the main executable.** Browser MCP functionality must run via `mergen-ade(.exe) --browser-mcp-helper`, not as a separate sidecar binary.
- **Do not ship a separate `mergen-browser-mcp(.exe)` binary.** Release ZIP/DMG must contain only the main Mergen executable; sidecar binaries are unsupported and must be removed.
- **Browser MCP helper code lives in the Electron main process.** The helper is implemented in `electron/main/browserMcpHelper.ts` and runs as a child process via `--browser-mcp-helper` flag.
- **OpenCode runtime config must use the helper-mode argument.** The MCP command array must be `[current_exe, "--browser-mcp-helper", "--caps=devtools,vision,network,storage"]`.
- **Release builds must target only `--bin mergen-ade`.** Do not use `--bins` in release workflows; it builds all binary targets including stale sidecars.
- **Clean stale `mergen-browser-mcp(.exe)` artifacts.** Release scripts must remove any existing sidecar executable from previous builds to prevent accidental packaging.
- **Helper mode runs headless before GUI initialization.** When `--browser-mcp-helper` is detected, run the MCP JSON-RPC loop and exit; skip all eframe/egui initialization, wgpu setup, and window creation.
- **Helper mode uses stdio pipes.** The helper reads JSON-RPC requests from stdin and writes responses to stdout; GUI subsystem executables on Windows still support stdio redirection via pipes.

## Browser MCP Multi-Terminal Isolation Guidelines
- **Browser instances must be terminal-scoped for MCP isolation.** Each terminal using the Browser MCP must have its own isolated WebView2 instance (`BrowserScopeKey::Terminal`) to prevent session conflicts when multiple AI agents control browsers in the same project simultaneously.
- **BrowserScopeKey enum distinguishes project vs terminal scope.** Use `BrowserScopeKey::Project(pid)` for legacy UI-initiated browser usage; use `BrowserScopeKey::Terminal { project_id, terminal_id }` for MCP-originated browser commands.
- **Terminal-scoped browsers share the project's WebView2 profile directory.** Terminal browsers store their WebView2 user data in `webview2/projects/{project_id}/` (via `browser_user_data_dir_path()`), the same folder as the project-scoped browser. This means passwords, cookies, localStorage, and session state are shared across all terminals in the same project. Terminal-scoped isolation remains for tabs, design inspect state, and video recordings, which are keyed by `BrowserScopeKey::Terminal`.
- **Project browsers remain for UI-initiated navigation.** When users click terminal HTTP links or manually open the browser panel, continue using project-scoped browsers (`BrowserScopeKey::Project`) to preserve the existing single-browser-per-project user experience.
- **Browser MCP commands always resolve to terminal scope.** The `resolve_browser_mcp_scope()` function returns `BrowserScopeKey::Terminal` based on authenticated `auth_scope.terminal_id`, ensuring MCP commands never share browser state between different terminal sessions.
- **Session ID validation prevents cross-session contamination.** Browser MCP requests must include the `session_id` from the auth scope; mismatch between request and auth scope session ID rejects the request to prevent session hopping attacks.
- **Browser state maps use BrowserScopeKey instead of raw project_id.** All browser state (tabs, URL drafts, embedded browser instances, design inspect state, video recordings) is keyed by `BrowserScopeKey` to support both project and terminal scopes uniformly.
- **UI panel shows active terminal's browser when available.** The browser panel displays the terminal-scoped browser when the active terminal has one open, falling back to project-scoped browser for terminals without terminal-specific browsers.
- **`active_browser_scope()` is the single source of truth for panel display.** Both `draw_browser_panel()` and `sync_embedded_browser()` must use `active_browser_scope()` (not hardcoded `BrowserScopeKey::Project`) to determine which browser instance to show, hide, and sync. This ensures terminal-scoped browsers created by MCP are visible in the panel and receive native bounds synchronization.
- **Terminal-scoped browser tabs must not trigger project URL persistence.** `set_browser_url_for_scope()` and `apply_browser_tab_observed_url()` must only write to `ProjectRecord::browser_last_url` for `BrowserScopeKey::Project`, never for `BrowserScopeKey::Terminal`.
- **Terminal browser cleanup occurs on terminal close.** When a terminal exits, its terminal-scoped browser state (tabs, WebView instance, recordings) must be cleaned up to prevent resource leaks.
- **Terminal-scoped browser URLs are not persisted.** Unlike project-scoped browsers that persist `browser_last_url` to config, terminal-scoped browser URLs are runtime-only and do not survive application restart.
- **Browser panel visible scope must follow the MCP-controlled terminal.** When an MCP command targets a terminal-scoped browser, the panel must display that terminal's browser even if the terminal is not the active one. Use `browser_panel_visible_scope_by_project: BTreeMap<u64, BrowserScopeKey>` (runtime-only) to pin the visible scope per project.
- **`active_browser_scope()` must check visible scope override first.** Priority: (1) explicit override from `browser_panel_visible_scope_by_project`, (2) active terminal's terminal-scoped browser if it has tabs, (3) project-scoped browser fallback. The override is set by MCP handlers (`prepare_browser_mcp_tool_scope`, `handle_browser_mcp_request`) and by terminal activation (`set_active_terminal`).
- **Visible scope override must be cleared on terminal activation if the new active terminal has no browser tabs.** This prevents stale overrides from keeping the panel on a closed browser when the user switches terminals.
- **Visible scope override must be cleaned up on project removal and last-tab-close.** In `remove_project()`, clear the project's visible scope entry. In `close_browser_tab()` when the last tab closes for a terminal scope, remove the override for that project.
- **Browser panel must not show a separate terminal/project scope selector.** Terminal-scoped browser isolation remains internal; users switch which terminal browser they are viewing by activating that terminal, while MCP-originated background browser work may still pin the visible scope through `browser_panel_visible_scope_by_project`.
- **OpenCode runtime config must disable external browser MCP servers.** In addition to enabling `mergen-browser`, the runtime `opencode.json` must explicitly disable known external browser MCP server names (`playwright`, `browser`, `puppeteer`) with `"enabled": false` to prevent OpenCode from falling back to its own Chrome/Playwright instance.

## Browser MCP Highlight Overlay Guidelines
- **`browser_highlight` must fail closed for clipped targets.** Do not draw highlight overlays for elements hidden by clipped/collapsed ancestors, closed sidebars, overlays, or hit-test coverage; return a clear error instead.
- **Highlight target geometry must use the painted/reachable rect.** Account for ancestor `overflow` clipping and WebView viewport clipping before moving the cursor or drawing a fixed-position overlay.
- **Highlight overlay geometry must stay inside the visible viewport.** Clamp overlay edges to `0..window.innerWidth` and `0..window.innerHeight` so sidebars and edge-aligned controls do not produce cut-off borders or labels.
- **Bump the injected automation script version whenever highlight visibility, hit-testing, clipping, or label-placement behavior changes.** Add regression tests for hidden sidebars, clipped ancestors, and edge-of-viewport highlights.

## Browser Design Inspect Guidelines

- **Design Inspect toolbar icon is `icons::PENCIL`.** The toggle button in the browser toolbar uses the Lucide pencil glyph. ON/OFF state is communicated through selected/highlight color, not by swapping between an eye and an eye-off icon.
- **Design Inspect delivery is click-only.** Hover and pointer movement may update the highlight overlay but must never send context to the terminal.
- **Design Inspect clicks must block page actions.** While inspect mode is enabled, selecting an element must prevent normal page click behavior such as link navigation, button handlers, and form submission.
- **Design Inspect auto-disables after successful delivery.** When a user clicks a page element and the design inspect info is successfully queued to the terminal, the mode is automatically disabled to prevent accidental duplicate clicks. Users must re-enable design inspect via the toolbar button to select another element.
- **Browser events must use selection semantics.** Use click/selection event names such as `DesignElementClicked`; do not reintroduce terminal forwarding from hover events.
- **Stale hover messages must fail closed.** Ignore `type: "hover"` design-inspect messages from old injected scripts instead of forwarding them to terminals.
- **Bump the injected script version when Design Inspect behavior changes.** This prevents an existing `window.__mergenDesignInspect` implementation from short-circuiting around newer behavior.
- **Add regression tests for Design Inspect behavior.** Cover click parsing, hover rejection, duplicate click dedupe, stale URL gating, iframe page URL gating, auto-disable after delivery, and user-facing enable/status copy.
