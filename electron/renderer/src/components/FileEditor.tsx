import React, { useState, useEffect, useCallback } from 'react';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface FileEditorProps {
  filePath: string;
  displayName: string;
  onClose?: () => void;
}

export const FileEditor: React.FC<FileEditorProps> = ({ filePath, displayName, onClose }) => {
  const [text, setText] = useState('');
  const [savedText, setSavedText] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      setLoading(true);
      setError(null);
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

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c0c' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>
          {displayName}
          {isDirty && <span style={{ color: '#e8a838', marginLeft: 4 }}>●</span>}
        </span>
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            onClick={save}
            disabled={!isDirty}
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
            <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
              ✕
            </button>
          )}
        </div>
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '8px 12px' }}>
        {loading && <div style={{ color: '#888', fontSize: 12 }}>Loading...</div>}
        {error && <div style={{ color: '#c44', fontSize: 12 }}>Error: {error}</div>}
        {!loading && !error && (
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            style={{
              width: '100%',
              height: '100%',
              minHeight: 200,
              background: '#0c0c0c',
              border: 'none',
              color: '#ccc',
              fontFamily: 'Consolas, "Courier New", monospace',
              fontSize: 13,
              lineHeight: 1.5,
              resize: 'none',
              outline: 'none',
            }}
            spellCheck={false}
          />
        )}
      </div>
    </div>
  );
};
