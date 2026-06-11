import { useState, useEffect, useCallback } from 'react';
import type { AcpChatSession, AcpChatMessage } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

export function useAcp(chatId: string) {
  const [session, setSession] = useState<AcpChatSession | null>(null);
  const [messages, setMessages] = useState<AcpChatMessage[]>([]);
  const [status, setStatus] = useState<string>('starting');

  const refreshSession = useCallback(async () => {
    const s = await api.invoke('acp:getSession', chatId) as AcpChatSession | null;
    setSession(s);
    if (s) {
      setMessages(s.messages);
      setStatus(s.status);
    }
  }, [chatId]);

  useEffect(() => {
    refreshSession();
    const unsub = api.on('acp:event', (eventChatId: string, event: { type: string; text?: string; options?: unknown; modeId?: string; commands?: unknown; requestId?: string; message?: string }) => {
      if (eventChatId !== chatId) return;
      if (event.type === 'promptResponse') {
        setMessages((prev) => [...prev, { role: 'assistant', text: event.text || '', timestamp: Date.now() }]);
        setStatus('idle');
      } else if (event.type === 'promptSent') {
        setMessages((prev) => [...prev, { role: 'user', text: event.text || '', timestamp: Date.now() }]);
        setStatus('running');
      } else if (event.type === 'sessionCreated') {
        setStatus('idle');
      } else if (event.type === 'error') {
        setMessages((prev) => [...prev, { role: 'system', text: `Error: ${event.text || 'Unknown'}`, timestamp: Date.now() }]);
      } else if (event.type === 'cancelled') {
        setStatus('idle');
      } else if (event.type === 'permission') {
        setStatus('permission');
      }
      refreshSession();
    });
    return () => { unsub(); };
  }, [chatId, refreshSession]);

  const sendPrompt = useCallback(async (promptText: string, attachments: string[] = [], modeId?: string) => {
    await api.invoke('acp:send', { chatId, promptText, attachments, modeId });
  }, [chatId]);

  const cancel = useCallback(async () => {
    await api.invoke('acp:cancel', chatId);
  }, [chatId]);

  const setConfigOption = useCallback(async (configId: string, value: string) => {
    await api.invoke('acp:setConfigOption', { chatId, configId, value });
  }, [chatId]);

  return {
    session,
    messages,
    status,
    refreshSession,
    sendPrompt,
    cancel,
    setConfigOption,
  };
}
