import React from 'react';

interface AcpErrorBoundaryProps {
  children: React.ReactNode;
  onClose?: () => void;
}

interface AcpErrorBoundaryState {
  error?: Error;
}

export function acpRenderErrorMessage(error: Error | undefined): string {
  return error?.message || 'Unexpected render error';
}

export class AcpErrorBoundary extends React.Component<AcpErrorBoundaryProps, AcpErrorBoundaryState> {
  state: AcpErrorBoundaryState = {};

  static getDerivedStateFromError(error: Error): AcpErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error): void {
    console.error('ACP panel render error:', error);
  }

  private retry = (): void => {
    this.setState({ error: undefined });
  };

  render(): React.ReactNode {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <div style={{ display: 'flex', flexDirection: 'column', width: '100%', height: '100%', background: '#0c0c0c', color: '#ccc' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222', flexShrink: 0 }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>OpenCode ACP</span>
          {this.props.onClose && (
            <button
              onClick={this.props.onClose}
              style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}
            >
              x
            </button>
          )}
        </div>
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 24 }}>
          <div style={{ width: 'min(560px, 100%)', border: '1px solid #2a2a2a', borderRadius: 8, background: '#151515', padding: 16 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: '#eee', marginBottom: 8 }}>ACP panel render failed</div>
            <div style={{ fontSize: 12, color: '#999', lineHeight: 1.5, wordBreak: 'break-word' }}>
              {acpRenderErrorMessage(this.state.error)}
            </div>
            <div style={{ display: 'flex', gap: 8, marginTop: 14 }}>
              <button
                onClick={this.retry}
                style={{ fontSize: 12, padding: '6px 12px', borderRadius: 4, border: '1px solid #333', background: '#1f3a4c', color: '#ccc', cursor: 'pointer' }}
              >
                Retry ACP panel
              </button>
              {this.props.onClose && (
                <button
                  onClick={this.props.onClose}
                  style={{ fontSize: 12, padding: '6px 12px', borderRadius: 4, border: '1px solid #333', background: 'transparent', color: '#888', cursor: 'pointer' }}
                >
                  Close ACP
                </button>
              )}
            </div>
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, padding: '0 12px 6px', fontSize: 11, color: '#666', flexShrink: 0 }}>
          <span>Local</span>
          <span>error</span>
        </div>
      </div>
    );
  }
}
