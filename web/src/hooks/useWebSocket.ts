import { useEffect, useRef, useCallback } from 'react';
import { ServerMessage, ClientMessage } from '../types';

const WS_PROTOCOL = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const WS_URL = `${WS_PROTOCOL}//${window.location.host}/api/ws`;

export type TerminalOutputHandler = (terminalId: number, data: Uint8Array) => void;

export function useWebSocket(
  token: string,
  onMessage: (msg: ServerMessage) => void,
  onConnectedChange: (c: boolean) => void,
  onTerminalOutput?: TerminalOutputHandler
) {
  const wsRef = useRef<WebSocket | null>(null);
  const onTerminalOutputRef = useRef(onTerminalOutput);
  onTerminalOutputRef.current = onTerminalOutput;

  useEffect(() => {
    if (!token) return;
    const ws = new WebSocket(`${WS_URL}?token=${encodeURIComponent(token)}`);
    wsRef.current = ws;

    ws.onopen = () => {
      onConnectedChange(true);
    };
    ws.onclose = () => {
      onConnectedChange(false);
    };
    ws.onmessage = (event) => {
      try {
        if (typeof event.data === 'string') {
          const msg: ServerMessage = JSON.parse(event.data);
          if (msg.kind === 'terminal_output') {
            onTerminalOutputRef.current?.(msg.terminal_id, new Uint8Array(msg.data));
          }
          onMessage(msg);
        }
      } catch (e) {
        console.error('WS parse error', e);
      }
    };

    return () => {
      ws.close();
    };
  }, [token]);

  const sendMessage = useCallback((msg: ClientMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  const sendBinary = useCallback((data: Uint8Array) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(data);
    }
  }, []);

  return { sendMessage, sendBinary };
}
