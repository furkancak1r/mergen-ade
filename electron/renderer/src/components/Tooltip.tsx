import React, { useRef, useEffect, useState } from 'react';

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
  delay = 500,
  position = 'top',
}) => {
  const [show, setShow] = useState(false);
  const [visible, setVisible] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  if (disabled || !text) return children;

  const positionStyle: React.CSSProperties = position === 'top'
    ? { bottom: 'calc(100% + 6px)', left: '50%', transform: 'translateX(-50%)' }
    : { top: 'calc(100% + 6px)', left: '50%', transform: 'translateX(-50%)' };

  return (
    <div
      ref={ref}
      style={{ position: 'relative', display: 'inline-flex' }}
      onMouseEnter={() => {
        timerRef.current = setTimeout(() => setVisible(true), delay);
        setShow(true);
      }}
      onMouseLeave={() => {
        if (timerRef.current) clearTimeout(timerRef.current);
        setShow(false);
        setVisible(false);
      }}
    >
      {children}
      {show && visible && (
        <div
          style={{
            position: 'absolute',
            ...positionStyle,
            background: '#1a1a1a',
            border: '1px solid #333',
            borderRadius: 4,
            padding: '4px 8px',
            fontSize: 11,
            color: '#ccc',
            whiteSpace: 'nowrap',
            zIndex: 1000,
            pointerEvents: 'none',
          }}
        >
          {text}
        </div>
      )}
    </div>
  );
};
