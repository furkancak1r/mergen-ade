import { describe, expect, it } from 'vitest';
import { AcpErrorBoundary, acpRenderErrorMessage } from './AcpErrorBoundary';

describe('AcpErrorBoundary', () => {
  it('uses a stable fallback message for missing render errors', () => {
    expect(acpRenderErrorMessage(undefined)).toBe('Unexpected render error');
  });

  it('preserves render error messages for the fallback panel', () => {
    expect(acpRenderErrorMessage(new Error('slash popup failed'))).toBe('slash popup failed');
  });

  it('stores render errors in boundary state', () => {
    const error = new Error('boom');
    expect(AcpErrorBoundary.getDerivedStateFromError(error)).toEqual({ error });
  });
});
