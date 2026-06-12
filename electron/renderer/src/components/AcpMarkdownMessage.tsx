import React from 'react';
import ReactMarkdown from 'react-markdown';
import remarkBreaks from 'remark-breaks';
import remarkGfm from 'remark-gfm';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

export const AcpMarkdownMessage: React.FC<{ text: string }> = ({ text }) => {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm, remarkBreaks]}
      components={{
        a: ({ href, children }) => (
          <a
            href={href}
            onClick={(event) => {
              if (!href) return;
              event.preventDefault();
              if (/^https?:\/\//i.test(href)) {
                api.invoke('shell:openExternal', href);
              }
            }}
          >
            {children}
          </a>
        ),
        code: ({ className, children, ...props }) => {
          const value = String(children ?? '');
          if (!className && !value.includes('\n')) {
            return <code className="acp-md-inline-code" {...props}>{value}</code>;
          }
          return <code className={className} {...props}>{value}</code>;
        },
      }}
    >
      {text}
    </ReactMarkdown>
  );
};
