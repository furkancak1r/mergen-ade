import React, { useState, useEffect, useCallback, useRef } from 'react';
import { cappedHoverText } from '../lib/mojibake';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface FileEditorProps {
  filePath: string;
  displayName: string;
  onClose?: () => void;
  onDirtyChange?: (dirty: boolean) => void;
}

export const FileEditor: React.FC<FileEditorProps> = ({ filePath, displayName, onClose, onDirtyChange }) => {
  const [text, setText] = useState('');
  const [savedText, setSavedText] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectionDragActive, setSelectionDragActive] = useState(false);
  const [visibleRows, setVisibleRows] = useState(25);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

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
    if (start === end) return;
    const selected = text.slice(start, end);
    navigator.clipboard.writeText(selected).catch(() => {});
  }, [text]);

  // Listen for native copy events on the textarea (Event::Copy equivalent)
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    const handleCopyEvent = (e: ClipboardEvent) => {
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      if (start === end) return;
      const selected = text.slice(start, end);
      e.clipboardData?.setData('text/plain', selected);
      e.preventDefault();
    };
    ta.addEventListener('copy', handleCopyEvent);
    return () => ta.removeEventListener('copy', handleCopyEvent);
  }, [text]);

  // Context menu handler
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    // Simple context menu: copy/paste
    const menu = document.createElement('div');
    menu.style.cssText = 'position:fixed;z-index:1000;background:#1a1a1a;border:1px solid #333;border-radius:4px;padding:4px 0;font-size:12px;color:#ccc;';
    menu.style.left = `${e.clientX}px`;
    menu.style.top = `${e.clientY}px`;

    const copyBtn = document.createElement('button');
    copyBtn.textContent = 'Copy';
    copyBtn.style.cssText = 'display:block;width:100%;text-align:left;padding:4px 12px;background:transparent;border:none;color:#ccc;cursor:pointer;font-size:12px;';
    copyBtn.onmouseenter = () => { copyBtn.style.background = '#333'; };
    copyBtn.onmouseleave = () => { copyBtn.style.background = 'transparent'; };
    copyBtn.onclick = () => {
      handleCopy();
      menu.remove();
    };
    menu.appendChild(copyBtn);

    const pasteBtn = document.createElement('button');
    pasteBtn.textContent = 'Paste';
    pasteBtn.style.cssText = 'display:block;width:100%;text-align:left;padding:4px 12px;background:transparent;border:none;color:#ccc;cursor:pointer;font-size:12px;';
    pasteBtn.onmouseenter = () => { pasteBtn.style.background = '#333'; };
    pasteBtn.onmouseleave = () => { pasteBtn.style.background = 'transparent'; };
    pasteBtn.onclick = async () => {
      try {
        const clip = await navigator.clipboard.readText();
        const ta = textareaRef.current;
        if (!ta) return;
        const start = ta.selectionStart;
        const end = ta.selectionEnd;
        const newText = text.slice(0, start) + clip + text.slice(end);
        setText(newText);
        requestAnimationFrame(() => {
          ta.selectionStart = ta.selectionEnd = start + clip.length;
        });
      } catch {
        // ignore
      }
      menu.remove();
    };
    menu.appendChild(pasteBtn);

    document.body.appendChild(menu);
    const close = () => { menu.remove(); document.removeEventListener('click', close); };
    requestAnimationFrame(() => {
      document.addEventListener('click', close);
    });
  }, [handleCopy, text]);

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
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222', flexShrink: 0 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>
          {cappedHoverText(displayName, 60)}
          {isDirty && <span style={{ color: '#e8a838', marginLeft: 4 }}>●</span>}
        </span>
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            onClick={save}
            disabled={!isDirty}
            title="Save (Ctrl+S)"
            style={{
              padding: '4px 12px',
              fontSize: 11,
              background: isDirty ? '#1f3a4c' : '#1a1a1a',
              border: '1px solid #333',
              color: '#ccc',
              borderRadius: 3,
              cursor: isDirty ? 'pointer' : 'not-allowed',
            }}
          >
            Save
          </button>
          {onClose && (
            <button
              onClick={onClose}
              title="Close"
              style={{
                background: 'transparent',
                border: 'none',
                color: '#888',
                cursor: 'pointer',
                fontSize: 14,
                width: 24,
                height: 24,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
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
            onMouseDown={() => setSelectionDragActive(true)}
            onKeyDown={(e) => {
              if (e.ctrlKey && e.key === 's') {
                e.preventDefault();
                save();
              }
              if (e.ctrlKey && e.key === 'c') {
                // Let native copy work, but also ensure selection is preserved
                handleCopy();
              }
            }}
            onContextMenu={handleContextMenu}
            style={{
              width: '100%',
              minHeight: '100%',
              background: '#0c0c0c',
              border: 'none',
              color: '#ccc',
              fontFamily: 'Consolas, "Courier New", monospace',
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
    </div>
  );
};
