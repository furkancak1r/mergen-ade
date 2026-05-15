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
  const tokenRef = useRef(token);
  tokenRef.current = token;
  const onMessageRef = useRef(onMessage);
  onMessageRef.current = onMessage;
  const onConnectedChangeRef = useRef(onConnectedChange);
  onConnectedChangeRef.current = onConnectedChange;
  const onTerminalOutputRef = useRef(onTerminalOutput);
  onTerminalOutputRef.current = onTerminalOutput;
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectDelayRef = useRef(1000);

  useEffect(() => {
    if (!tokenRef.current) return;

    const connect = () => {
      const ws = new WebSocket(`${WS_URL}?token=${encodeURIComponent(tokenRef.current)}`);
      wsRef.current = ws;

      ws.onopen = () => {
        reconnectDelayRef.current = 1000;
        onConnectedChangeRef.current(true);
      };

      ws.onclose = () => {
        onConnectedChangeRef.current(false);
        // Auto-reconnect with exponential backoff (max 30s)
        reconnectDelayRef.current = Math.min(reconnectDelayRef.current * 2, 30000);
        reconnectTimerRef.current = setTimeout(connect, reconnectDelayRef.current);
      };

      ws.onmessage = (event) => {
        try {
          if (typeof event.data === 'string') {
            const msg: ServerMessage = JSON.parse(event.data);
            if (msg.kind === 'terminal_output') {
              onTerminalOutputRef.current?.(msg.terminal_id, new Uint8Array(msg.data));
            }
            onMessageRef.current(msg);
          } else if (event.data instanceof ArrayBuffer) {
            const buf = new Uint8Array(event.data);
            if (buf.length >= 8) {
              const view = new DataView(buf.buffer);
              const terminalId = Number(view.getBigUint64(0, true));
              const payload = buf.slice(8);
              onTerminalOutputRef.current?.(terminalId, payload);
            }
          } else if (event.data instanceof Blob) {
            const reader = new FileReader();
            reader.onload = () => {
              const buf = new Uint8Array(reader.result as ArrayBuffer);
              if (buf.length >= 8) {
                const view = new DataView(buf.buffer);
                const terminalId = Number(view.getBigUint64(0, true));
                const payload = buf.slice(8);
                onTerminalOutputRef.current?.(terminalId, payload);
              }
            };
            reader.readAsArrayBuffer(event.data);
          }
        } catch (e) {
          console.error('WS parse error', e);
        }
      };

      ws.onerror = (err) => {
        console.error('WebSocket error', err);
      };
    };

    connect();

    return () => {
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      if (wsRef.current) {
        wsRef.current.onclose = null;
        wsRef.current.close();
      }
    };
  }, []);

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
