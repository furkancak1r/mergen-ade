import React, { useState, useEffect, useRef } from 'react';
import { PanelHeader, Button, Input } from '../components/ui';

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
      <div style={{ padding: 'var(--space-sm) var(--space-md)', borderBottom: '1px solid var(--border-subtle)', display: 'flex', gap: 'var(--space-sm)', flexShrink: 0 }}>
        <Input
          value={inputUrl}
          onChange={e => setInputUrl(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleNavigate()}
          placeholder="Enter URL…"
          style={{ flex: 1 }}
        />
        <Button variant="primary" onClick={handleNavigate}>
          Go
        </Button>
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
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--text-muted)', fontSize: 'var(--font-base)' }}>
            Enter a URL to browse
          </div>
        )}
      </div>
    </div>
  );
};
