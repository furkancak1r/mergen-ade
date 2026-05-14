import React, { useState, useEffect, useRef } from 'react';

interface Props {
  url: string;
  onUrlChange: (url: string) => void;
}

export const BrowserPanel: React.FC<Props> = ({ url, onUrlChange }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [inputUrl, setInputUrl] = useState(url);

  useEffect(() => {
    setInputUrl(url);
  }, [url]);

  const handleNavigate = () => {
    let u = inputUrl.trim();
    if (!u) return;
    if (!u.startsWith('http://') && !u.startsWith('https://')) {
      u = 'https://' + u;
    }
    onUrlChange(u);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ padding: 6, borderBottom: '1px solid #333', display: 'flex', gap: 4 }}>
        <input
          value={inputUrl}
          onChange={e => setInputUrl(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleNavigate()}
          style={{ flex: 1, background: '#1a1a1a', border: '1px solid #444', color: '#e0e0e0', fontSize: 12, padding: '2px 6px' }}
        />
        <button
          onClick={handleNavigate}
          style={{ background: '#1e3a5f', border: '1px solid #4fc3f7', color: '#4fc3f7', fontSize: 11, cursor: 'pointer', padding: '2px 8px' }}
        >
          Go
        </button>
      </div>
      <div style={{ flex: 1, overflow: 'hidden' }}>
        {url ? (
          <iframe
            ref={iframeRef}
            src={url}
            style={{ width: '100%', height: '100%', border: 'none', background: '#fff' }}
            sandbox="allow-scripts allow-same-origin allow-forms"
            referrerPolicy="strict-origin-when-cross-origin"
            title="Browser Panel"
          />
        ) : (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#666', fontSize: 12 }}>
            Enter a URL to browse
          </div>
        )}
      </div>
    </div>
  );
};
