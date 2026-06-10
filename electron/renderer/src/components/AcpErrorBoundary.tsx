import React from 'react';

interface AcpErrorBoundaryProps {
  children: React.ReactNode;
  onClose?: () => void;
}

interface AcpErrorBoundaryState {
  error?: Error;
}

export class AcpErrorBoundary extends React.Component<AcpErrorBoundaryProps, AcpErrorBoundaryState> {
  state: AcpErrorBoundaryState = {};

  static getDerivedStateFromError(error: Error): AcpErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error): void {
    console.error('ACP panel render error:', error);
  }

  render(): React.ReactNode {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <div style={{ display: 'flex', flexDirection: 'column', width: '100%', height: '100%', background: '#0c0c0c', color: '#ccc', padding: 24, gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: '#eee' }}>ACP panel failed to render.</div>
        <div style={{ fontSize: 12, color: '#888', maxWidth: 640, lineHeight: 1.5 }}>
          {this.state.error.message || 'Unexpected render error'}
        </div>
        {this.props.onClose && (
          <button
            onClick={this.props.onClose}
            style={{ alignSelf: 'flex-start', fontSize: 12, padding: '6px 12px', borderRadius: 4, border: '1px solid #333', background: '#1a1a1a', color: '#ccc', cursor: 'pointer' }}
          >
            Close ACP
          </button>
        )}
      </div>
    );
  }
}
