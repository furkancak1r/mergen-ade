import React, { useState, useEffect, useCallback, useRef } from 'react';
import { cappedHoverText } from '../lib/mojibake';
import { selectedTextFromRange } from '../lib/fileEditor';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface FileEditorProps {
  filePath: string;
  displayName: string;
  canNavigateBack?: boolean;
  canNavigateForward?: boolean;
  onNavigateBack?: () => void;
  onNavigateForward?: () => void;
  onClose?: () => void;
  onDirtyChange?: (dirty: boolean) => void;
}

export const FileEditor: React.FC<FileEditorProps> = ({
  filePath,
  displayName,
  canNavigateBack = false,
  canNavigateForward = false,
  onNavigateBack,
  onNavigateForward,
  onClose,
  onDirtyChange,
}) => {
  const [text, setText] = useState('');
  const [savedText, setSavedText] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectionDragActive, setSelectionDragActive] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; selectedText: string | null; selectionStart: number; selectionEnd: number } | null>(null);
  const [visibleRows, setVisibleRows] = useState(25);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const secondaryClickSelectionRef = useRef<{ start: number; end: number } | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      setLoading(true);
      setError(null);
      setSelectionDragActive(false);
      try {
        const content = await api.invoke('fs:readFile', filePath) as string;
        if (!cancelled) {
          setText(content);
          setSavedText(content);
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    load();
    return () => { cancelled = true; };
  }, [filePath]);

  const save = useCallback(async () => {
    try {
      await api.invoke('fs:writeFile', filePath, text);
      setSavedText(text);
    } catch (err) {
      setError(String(err));
    }
  }, [filePath, text]);

  const isDirty = text !== savedText;
  const backEnabled = Boolean(onNavigateBack && canNavigateBack && !isDirty);
  const forwardEnabled = Boolean(onNavigateForward && canNavigateForward && !isDirty);
  const backTitle = isDirty ? 'Save changes before navigating back' : canNavigateBack ? 'Back' : 'No previous file';
  const forwardTitle = isDirty ? 'Save changes before navigating forward' : canNavigateForward ? 'Forward' : 'No next file';

  useEffect(() => {
    onDirtyChange?.(isDirty);
  }, [isDirty, onDirtyChange]);

  // Focus isolation: ensure editor owns focus when visible
  useEffect(() => {
    if (!loading && !error && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [loading, error]);

  // Compute visible rows from container height via ResizeObserver
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const h = entry.contentRect.height;
        const lineHeight = 13 * 1.5; // fontSize * lineHeight
        const rows = Math.max(1, Math.floor(h / lineHeight));
        setVisibleRows(rows);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Clamp horizontal scroll offset to 0 after each scroll event
  useEffect(() => {
    if (!containerRef.current) return;
    const handleScroll = () => {
      if (containerRef.current && containerRef.current.scrollLeft !== 0) {
        containerRef.current.scrollLeft = 0;
      }
    };
    containerRef.current.addEventListener('scroll', handleScroll);
    return () => {
      containerRef.current?.removeEventListener('scroll', handleScroll);
    };
  }, []);

  // Handle copy with Event::Copy detection + fallback
  const handleCopy = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    const start = ta.selectionStart;
    const end = ta.selectionEnd;
    const selected = selectedTextFromRange(text, start, end);
    if (!selected) return;
    navigator.clipboard.writeText(selected).catch(() => {});
  }, [text]);

  // Listen for native copy events on the textarea (Event::Copy equivalent)
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    const handleCopyEvent = (e: ClipboardEvent) => {
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      const selected = selectedTextFromRange(text, start, end);
      if (!selected) return;
      e.clipboardData?.setData('text/plain', selected);
      e.preventDefault();
    };
    ta.addEventListener('copy', handleCopyEvent);
    return () => ta.removeEventListener('copy', handleCopyEvent);
  }, [text]);

  useEffect(() => {
    if (!contextMenu) return undefined;

    const closeMenu = () => {
      setContextMenu(null);
      secondaryClickSelectionRef.current = null;
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('click', closeMenu);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('click', closeMenu);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [contextMenu]);

  const handleMouseDown = useCallback((event: React.MouseEvent<HTMLTextAreaElement>) => {
    const ta = textareaRef.current;
    if (event.button === 2) {
      if (ta && selectedTextFromRange(text, ta.selectionStart, ta.selectionEnd)) {
        secondaryClickSelectionRef.current = { start: ta.selectionStart, end: ta.selectionEnd };
      } else {
        secondaryClickSelectionRef.current = null;
      }
      return;
    }
    setSelectionDragActive(true);
  }, [text]);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const ta = textareaRef.current;
    if (!ta) return;
    const stored = secondaryClickSelectionRef.current;
    const selectionStart = stored?.start ?? ta.selectionStart;
    const selectionEnd = stored?.end ?? ta.selectionEnd;
    const selectedText = selectedTextFromRange(text, selectionStart, selectionEnd);
    setContextMenu({ x: e.clientX, y: e.clientY, selectedText, selectionStart, selectionEnd });
    requestAnimationFrame(() => {
      if (selectedText && textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.selectionStart = selectionStart;
        textareaRef.current.selectionEnd = selectionEnd;
      }
    });
  }, [text]);

  // Drag selection edge autoscroll with document-level tracking
  useEffect(() => {
    if (!selectionDragActive) return;
    const handleMouseMove = (e: MouseEvent) => {
      if (!containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const edgeSize = 40;
      const topEdge = rect.top + edgeSize;
      const bottomEdge = rect.bottom - edgeSize;
      const y = e.clientY;

      let delta = 0;
      if (y < topEdge) {
        delta = -Math.min(8, Math.max(1, Math.round((topEdge - y) / 5)));
      } else if (y > bottomEdge) {
        delta = Math.min(8, Math.max(1, Math.round((y - bottomEdge) / 5)));
      }

      if (delta !== 0) {
        containerRef.current.scrollTop = Math.max(0, containerRef.current.scrollTop + delta);
      }
    };
    const handleMouseUp = () => {
      setSelectionDragActive(false);
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [selectionDragActive]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c0c' }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222', flexShrink: 0, gap: 8 }}>
        <div className="file-editor-title-group">
          <div className="file-editor-nav-group" aria-label="File editor navigation">
            <button
              className="file-editor-toolbar-btn"
              onClick={onNavigateBack}
              disabled={!backEnabled}
              title={backTitle}
              aria-label="Back"
            >
              ←
            </button>
            <button
              className="file-editor-toolbar-btn"
              onClick={onNavigateForward}
              disabled={!forwardEnabled}
              title={forwardTitle}
              aria-label="Forward"
            >
              →
            </button>
          </div>
          <span className="file-editor-title" title={displayName}>
            {cappedHoverText(displayName, 60)}
            {isDirty && <span style={{ color: '#e8a838', marginLeft: 4 }}>●</span>}
          </span>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            className={`file-editor-toolbar-btn ${isDirty ? '' : 'muted'}`}
            onClick={save}
            disabled={!isDirty}
            title={isDirty ? 'Save File (Ctrl+S)' : 'No unsaved changes'}
          >
            ✓
          </button>
          {onClose && (
            <button
              className="file-editor-toolbar-btn"
              onClick={onClose}
              title="Close Editor"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      {/* Editor area */}
      <div
        ref={containerRef}
        style={{ flex: 1, overflow: 'auto', padding: '8px 12px', position: 'relative' }}
      >
        {loading && <div style={{ color: '#888', fontSize: 12 }}>Loading...</div>}
        {error && <div style={{ color: '#c44', fontSize: 12 }}>Error: {error}</div>}
        {!loading && !error && (
          <textarea
            ref={textareaRef}
            value={text}
            rows={Math.max(visibleRows, text.split('\n').length)}
            onChange={(e) => setText(e.target.value)}
            onMouseDown={handleMouseDown}
            onKeyDown={(e) => {
              if (e.ctrlKey && e.key === 's') {
                e.preventDefault();
                save();
              }
              // Ctrl+C is handled by the native 'copy' event listener below
            }}
            onContextMenu={handleContextMenu}
            style={{
              width: '100%',
              minHeight: '100%',
              background: '#0c0c0c',
              border: 'none',
              color: '#ccc',
              fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, "Courier New", monospace',
              fontSize: 13,
              lineHeight: 1.5,
              resize: 'none',
              outline: 'none',
              whiteSpace: 'pre',
              overflowWrap: 'normal',
              overflowX: 'auto',
            }}
            spellCheck={false}
            data-editor-input
          />
        )}
      </div>
      {contextMenu && (
        <div
          className="file-editor-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          <button
            type="button"
            disabled={!contextMenu.selectedText}
            onClick={() => {
              if (!contextMenu.selectedText) return;
              navigator.clipboard.writeText(contextMenu.selectedText).catch(() => {});
              setContextMenu(null);
              secondaryClickSelectionRef.current = null;
              requestAnimationFrame(() => {
                if (textareaRef.current) {
                  textareaRef.current.focus();
                  textareaRef.current.selectionStart = contextMenu.selectionStart;
                  textareaRef.current.selectionEnd = contextMenu.selectionEnd;
                }
              });
            }}
          >
            Copy
          </button>
        </div>
      )}
    </div>
  );
};
