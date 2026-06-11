# Directory Indexing & Search Guidelines

## Directory Indexing Performance Guidelines
- Directory tree indexing must never block the UI thread. Do not use blocking sends or synchronous recursive filesystem scans from egui rendering paths.
- **Initial indexing must be shallow**: only read the root directory's immediate children. All child directories must be deferred regardless of name; do not recursively scan any directory during initial project open.
- **Lazy subtree loading must be one-level-only**: when a deferred directory is expanded, load only its immediate children. Child directories discovered during this load must also be deferred.
- Use `DirectoryScanMode::InitialRoot` for the first project scan and `DirectoryScanMode::LazySubtree` for on-demand expansion. Do not use boolean `allow_defer` flags.
- Large/generated directories such as `.git`, `target`, `node_modules`, `.next`, `dist`, `build`, caches, and virtual environments are automatically deferred by the shallow scan behavior.
- Time budgets must be enforced as **hard stops** inside entry iteration and child construction loops, not just at function boundaries. Check `should_stop()` before every expensive operation.
- Prefer `DirEntry::file_type()` over `fs::symlink_metadata(path)` and `path.is_dir()` to minimize filesystem calls.
- Prefer fast partial snapshots over waiting for a complete tree. The Directory panel should become usable immediately.
- `partial_warning` should remain internal state only; do not display it in the Directory panel UI.
- Preserve symlink safeguards: never recursively descend into symlinked directories.
- The worker thread should drain stale commands and prefer the latest `Full` command per project to avoid processing outdated requests.
- **Deferred directories must use `DirectoryNode::is_deferred` as metadata**; do not add visible placeholder children for normal lazy-load state. Placeholder nodes (`directory_placeholder_node`) are only for exceptional/truncated states such as load failure, outside-project paths, or omitted items after hard limits.
- **Directory worker command draining must never silently drop distinct `Subtree` commands**. Use batch draining (`Vec<DirectoryIndexCommand>`) to preserve all subtree load requests. Only deduplicate Full commands per project (keep latest generation).
- **When the UI queues a subtree load, request a repaint** (`request_repaint_after`) to process worker events promptly without waiting for unrelated input.
- **`request_directory_subtree_load()` must report whether work was queued** (`bool`) and must clean up loading state (`directory_index_subtree_loading_by_project`) if command send fails, to prevent stuck loading indicators.
- **Directory search must progressively queue deferred directories** even when the folder name itself does not match the query; otherwise matches inside lazy-loaded folders can never be discovered.
- **Search-triggered directory loading must still use `DirectoryScanMode::LazySubtree`**; never perform synchronous recursive scans from the UI thread.
- **While deferred search loads are queued, in flight, or waiting for debounce, do not show final "No matching files or folders" feedback.** Instead show a "Searching folders..." indicator and schedule repaint to continue loading.
- **Cap search-triggered subtree queueing per frame** (`DIRECTORY_SEARCH_INITIAL_SUBTREE_REQUESTS_PER_FRAME` and `DIRECTORY_SEARCH_BACKGROUND_SUBTREE_REQUESTS_PER_FRAME`) to keep large projects responsive; defer additional directories in subsequent frames via repaint.
- **Debounce search-triggered deferred loading and self-wake**: Wait `DIRECTORY_SEARCH_DEFERRED_LOAD_DEBOUNCE_SECS` (250ms) after query stops changing before starting deep deferred loads. Schedule `request_repaint_after` for the remaining debounce duration so loading starts promptly without depending on unrelated input or project switching.
- **Minimum query length for deferred loading is character-based, not byte-based**: Use `query.chars().count()` against `DIRECTORY_SEARCH_MIN_DEFERRED_QUERY_CHARS` (2 characters) so Unicode searches respect the same minimum length threshold as ASCII.
- **Adaptive per-frame loading caps**: Use aggressive cap (`DIRECTORY_SEARCH_INITIAL_SUBTREE_REQUESTS_PER_FRAME` = 8) when no results exist yet, conservative cap (`DIRECTORY_SEARCH_BACKGROUND_SUBTREE_REQUESTS_PER_FRAME` = 2) when results already visible. This prioritizes finding first matches without overwhelming UI.
- **Hidden deferred queue for search**: Deferred directories whose names don't match the query should be loaded in background without being added to `matching_directories`. Only directories that actually contain matches should be expanded in UI; others load hidden.
- **Directory search results must update automatically as deferred subtree results arrive.** Compute visible paths from the current snapshot each frame; do not require explicit user action to refresh results.
- **Do not add "New results found" / "Update results" UI for directory search.** Users expect search results to appear automatically as background loading completes.
- **Do not conflate parent visibility with descendant visibility.** A parent directory shown because it contains a matching file must not force all sibling descendants visible. Only directories whose own names match the query should force-show descendants.
- **Directory search result highlighting must be char-safe.** Highlight matched query text with a high-contrast orange color (`DIRECTORY_SEARCH_MATCH_COLOR`) in file and folder names. Use `LayoutJob` with multiple `TextFormat` sections to apply highlighting. Always use byte ranges derived from lowercase string indices to avoid splitting multi-byte UTF-8 sequences. Preserve row ordering, lazy loading behavior, and tooltips while adding visual feedback.
- **Directory search query tracking must run before snapshot availability checks.** Update debounce/query state even while the selected project's directory index is missing, loading, or errored; do not wait for a fully loaded snapshot before arming search repaint/deferred-load logic.
- **Project selection changes must reset Directory search tracking, not the user's query text.** Preserve `directory_search_query`, but reset project-scoped debounce/last-query state so the same query re-runs for the newly selected project.
- **Directory search input focus must not be stolen by AI attention routing.** When Directory search owns keyboard focus, text input belongs to the search field until the user explicitly focuses a terminal.
- Add regression tests whenever directory indexing, deferred loading, or tree rendering behavior changes.

## Directory Icons Guidelines
- **Directory rows must include stable icons.** File and folder rows should render IDE-like icons without changing lazy loading, search filtering, or row ordering behavior.
- **Directory file icons are extension-based only.** Do not add blocking metadata reads just to choose icons; use the existing `DirectoryNode` path/name data.
- **Directory search highlighting must remain char-safe with icons.** Adding icons must not alter UTF-8-safe match highlighting or split multi-byte characters.
