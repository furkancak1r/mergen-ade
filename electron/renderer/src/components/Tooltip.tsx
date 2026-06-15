import React, { useRef, useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { viewportTooltipPosition, type TooltipAnchorRect, type TooltipPlacement, type TooltipHorizontalAlign } from '../lib/tooltipPosition';

interface TooltipProps {
  children: React.ReactElement;
  text: string;
  disabled?: boolean;
  delay?: number;
  position?: 'top' | 'bottom';
}

export const Tooltip: React.FC<TooltipProps> = ({
  children,
  text,
  disabled = false,
  delay = 1000,
  position = 'top',
}) => {
  const [anchorRect, setAnchorRect] = useState<TooltipAnchorRect | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  if (disabled || !text) return children;

  return (
    <div
      ref={ref}
      style={{ position: 'relative', display: 'inline-flex' }}
      onMouseEnter={() => {
        timerRef.current = setTimeout(() => {
          if (ref.current) setAnchorRect(domRectToTooltipRect(ref.current.getBoundingClientRect()));
        }, delay);
      }}
      onMouseMove={() => {
        if (anchorRect && ref.current) {
          setAnchorRect(domRectToTooltipRect(ref.current.getBoundingClientRect()));
        }
      }}
      onMouseLeave={() => {
        if (timerRef.current) clearTimeout(timerRef.current);
        setAnchorRect(null);
      }}
    >
      {children}
      {anchorRect && <ViewportTooltipBubble text={text} rect={anchorRect} placement={position} />}
    </div>
  );
};

export const GlobalTooltip: React.FC<{ delay?: number }> = ({ delay = 1000 }) => {
  const [tooltip, setTooltip] = useState<{ text: string; rect: TooltipAnchorRect; placement: TooltipPlacement; horizontalAlign: TooltipHorizontalAlign } | null>(null);
  const anchorRef = useRef<HTMLElement | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const clearTimer = () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };

    const hide = () => {
      clearTimer();
      anchorRef.current = null;
      setTooltip(null);
    };

    const resolveHorizontalAlign = (anchor: HTMLElement): TooltipHorizontalAlign => {
      if (anchor.hasAttribute('data-tooltip-right')) return 'left-edge';
      if (anchor.hasAttribute('data-tooltip-left')) return 'right-edge';
      return 'center';
    };

    const showForAnchor = (anchor: HTMLElement) => {
      clearTimer();
      anchorRef.current = anchor;
      const text = anchor.getAttribute('data-tooltip') ?? '';
      const placement = anchor.hasAttribute('data-tooltip-bottom') ? 'bottom' : 'top';
      const horizontalAlign = resolveHorizontalAlign(anchor);
      timerRef.current = setTimeout(() => {
        if (anchorRef.current !== anchor || !anchor.isConnected || !text) return;
        setTooltip({ text, rect: domRectToTooltipRect(anchor.getBoundingClientRect()), placement, horizontalAlign });
      }, delay);
    };

    const handleMouseOver = (event: MouseEvent) => {
      const anchor = tooltipAnchorFromTarget(event.target);
      if (!anchor || anchor === anchorRef.current) return;
      showForAnchor(anchor);
    };

    const handleMouseMove = (event: MouseEvent) => {
      const anchor = tooltipAnchorFromTarget(event.target);
      if (!anchor || anchor !== anchorRef.current) return;
      setTooltip((current) => current
        ? { ...current, rect: domRectToTooltipRect(anchor.getBoundingClientRect()) }
        : current);
    };

    const handleMouseOut = (event: MouseEvent) => {
      const anchor = anchorRef.current;
      if (!anchor) return;
      const related = event.relatedTarget;
      if (related instanceof Node && anchor.contains(related)) return;
      hide();
    };

    const handleViewportChange = () => {
      const anchor = anchorRef.current;
      if (!anchor || !anchor.isConnected) {
        hide();
        return;
      }
      setTooltip((current) => current
        ? { ...current, rect: domRectToTooltipRect(anchor.getBoundingClientRect()) }
        : current);
    };

    document.addEventListener('mouseover', handleMouseOver);
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseout', handleMouseOut);
    window.addEventListener('scroll', handleViewportChange, true);
    window.addEventListener('resize', handleViewportChange);

    return () => {
      clearTimer();
      document.removeEventListener('mouseover', handleMouseOver);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseout', handleMouseOut);
      window.removeEventListener('scroll', handleViewportChange, true);
      window.removeEventListener('resize', handleViewportChange);
    };
  }, [delay]);

  if (!tooltip) return null;
  return <ViewportTooltipBubble text={tooltip.text} rect={tooltip.rect} placement={tooltip.placement} horizontalAlign={tooltip.horizontalAlign} />;
};

function ViewportTooltipBubble({ text, rect, placement, horizontalAlign = 'center' }: { text: string; rect: TooltipAnchorRect; placement: TooltipPlacement; horizontalAlign?: TooltipHorizontalAlign }) {
  if (typeof document === 'undefined') return null;
  const position = viewportTooltipPosition(rect, {
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
    preferredPlacement: placement,
    horizontalAlign,
  });

  const translateX = horizontalAlign === 'center' ? '-50%' : '0';
  const translateY = position.placement === 'top' ? '-100%' : '0';

  return createPortal(
    <div
      className="viewport-tooltip"
      style={{
        left: position.left,
        top: position.top,
        maxWidth: position.maxWidth,
        transform: `translate(${translateX}, ${translateY})`,
      }}
    >
      {text}
    </div>,
    document.body,
  );
}

function domRectToTooltipRect(rect: DOMRect): TooltipAnchorRect {
  return {
    top: rect.top,
    right: rect.right,
    bottom: rect.bottom,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}

function tooltipAnchorFromTarget(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) return null;
  const anchor = target.closest('[data-tooltip]');
  if (!(anchor instanceof HTMLElement)) return null;
  return anchor.getAttribute('data-tooltip') ? anchor : null;
}
