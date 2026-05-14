import React from 'react';

/* ── Primitive styled components used across all panels ── */

export const PanelHeader: React.FC<{
  title: string;
  children?: React.ReactNode;
}> = ({ title, children }) => (
  <div
    style={{
      padding: 'var(--space-md) var(--space-lg)',
      borderBottom: '1px solid var(--border-subtle)',
      display: 'flex',
      alignItems: 'center',
      gap: 'var(--space-sm)',
      flexShrink: 0,
    }}
  >
    <strong
      style={{
        fontSize: 'var(--font-base)',
        color: 'var(--text-secondary)',
        flex: 1,
        textTransform: 'uppercase',
        letterSpacing: '0.5px',
        fontWeight: 600,
      }}
    >
      {title}
    </strong>
    {children}
  </div>
);

export const ScrollArea: React.FC<{
  children: React.ReactNode;
  maxHeight?: number | string;
}> = ({ children, maxHeight }) => (
  <div
    style={{
      flex: 1,
      overflow: 'auto',
      maxHeight,
    }}
  >
    {children}
  </div>
);

export const EmptyState: React.FC<{ message: string }> = ({ message }) => (
  <div
    style={{
      padding: 'var(--space-lg)',
      fontSize: 'var(--font-sm)',
      color: 'var(--text-muted)',
      textAlign: 'center',
    }}
  >
    {message}
  </div>
);

export const LoadingState: React.FC = () => (
  <div
    style={{
      padding: 'var(--space-md)',
      fontSize: 'var(--font-sm)',
      color: 'var(--text-secondary)',
    }}
  >
    Loading…
  </div>
);

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'active';

export const Button: React.FC<{
  variant?: ButtonVariant;
  onClick?: React.MouseEventHandler<HTMLButtonElement>;
  children: React.ReactNode;
  title?: string;
  style?: React.CSSProperties;
  disabled?: boolean;
  type?: 'button' | 'submit';
}> = ({ variant = 'ghost', onClick, children, title, style, disabled, type = 'button' }) => {
  const base: React.CSSProperties = {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 'var(--space-xs)',
    borderRadius: 'var(--radius-md)',
    cursor: disabled ? 'not-allowed' : 'pointer',
    fontSize: 'var(--font-sm)',
    fontWeight: 500,
    padding: 'var(--space-xs) var(--space-sm)',
    minHeight: 28,
    minWidth: 28,
    transition: 'background 0.12s, border-color 0.12s, color 0.12s',
    lineHeight: 1,
    whiteSpace: 'nowrap',
    opacity: disabled ? 0.5 : 1,
  };

  const variants: Record<ButtonVariant, React.CSSProperties> = {
    primary: {
      background: 'var(--accent-dim)',
      border: '1px solid var(--accent)',
      color: 'var(--accent)',
    },
    secondary: {
      background: 'var(--bg-button-secondary)',
      border: '1px solid var(--border-default)',
      color: 'var(--text-secondary)',
    },
    ghost: {
      background: 'transparent',
      border: '1px solid transparent',
      color: 'var(--text-secondary)',
    },
    danger: {
      background: 'transparent',
      border: '1px solid transparent',
      color: 'var(--danger)',
    },
    active: {
      background: 'var(--bg-active)',
      border: '1px solid var(--accent)',
      color: 'var(--accent)',
    },
  };

  const hoverBg: Record<ButtonVariant, string> = {
    primary: 'rgba(79, 195, 247, 0.25)',
    secondary: 'var(--bg-hover)',
    ghost: 'var(--bg-hover)',
    danger: 'rgba(244, 67, 54, 0.12)',
    active: 'var(--bg-active)',
  };

  return (
    <button
      type={type}
      title={title}
      onClick={onClick}
      disabled={disabled}
      style={{
        ...base,
        ...variants[variant],
        ...style,
      }}
      onMouseEnter={e => {
        if (!disabled) {
          (e.currentTarget as HTMLButtonElement).style.background = hoverBg[variant];
        }
      }}
      onMouseLeave={e => {
        (e.currentTarget as HTMLButtonElement).style.background = String(variants[variant].background ?? 'transparent');
      }}
    >
      {children}
    </button>
  );
};

export const Input: React.FC<{
  value: string;
  onChange: React.ChangeEventHandler<HTMLInputElement>;
  placeholder?: string;
  style?: React.CSSProperties;
  onKeyDown?: React.KeyboardEventHandler<HTMLInputElement>;
  type?: string;
}> = ({ value, onChange, placeholder, style, onKeyDown, type = 'text' }) => (
  <input
    type={type}
    value={value}
    onChange={onChange}
    placeholder={placeholder}
    onKeyDown={onKeyDown}
    style={{
      background: 'var(--bg-input)',
      border: '1px solid var(--border-default)',
      color: 'var(--text-primary)',
      fontSize: 'var(--font-sm)',
      padding: 'var(--space-xs) var(--space-sm)',
      borderRadius: 'var(--radius-sm)',
      outline: 'none',
      width: '100%',
      ...style,
    }}
    onFocus={e => {
      e.currentTarget.style.borderColor = 'var(--border-focus)';
    }}
    onBlur={e => {
      e.currentTarget.style.borderColor = 'var(--border-default)';
    }}
  />
);

export const TextArea: React.FC<{
  value: string;
  onChange: React.ChangeEventHandler<HTMLTextAreaElement>;
  placeholder?: string;
  rows?: number;
  maxLength?: number;
  style?: React.CSSProperties;
  onKeyDown?: React.KeyboardEventHandler<HTMLTextAreaElement>;
}> = ({ value, onChange, placeholder, rows = 1, maxLength, style, onKeyDown }) => (
  <textarea
    value={value}
    onChange={onChange}
    placeholder={placeholder}
    rows={rows}
    maxLength={maxLength}
    onKeyDown={onKeyDown}
    style={{
      background: 'var(--bg-input)',
      border: '1px solid var(--border-default)',
      color: 'var(--text-primary)',
      fontSize: 'var(--font-base)',
      padding: 'var(--space-xs) var(--space-sm)',
      borderRadius: 'var(--radius-sm)',
      outline: 'none',
      resize: 'vertical',
      minHeight: 28,
      maxHeight: 120,
      fontFamily: 'inherit',
      width: '100%',
      ...style,
    }}
    onFocus={e => {
      e.currentTarget.style.borderColor = 'var(--border-focus)';
    }}
    onBlur={e => {
      e.currentTarget.style.borderColor = 'var(--border-default)';
    }}
  />
);

export const Icon: React.FC<{
  symbol: string;
  size?: number;
  color?: string;
  style?: React.CSSProperties;
}> = ({ symbol, size = 14, color = 'var(--text-secondary)', style }) => (
  <span
    style={{
      width: size,
      height: size,
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      fontSize: size,
      lineHeight: 1,
      color,
      flexShrink: 0,
      ...style,
    }}
  >
    {symbol}
  </span>
);

export const Row: React.FC<{
  children: React.ReactNode;
  active?: boolean;
  exited?: boolean;
  onClick?: React.MouseEventHandler<HTMLDivElement>;
  style?: React.CSSProperties;
}> = ({ children, active, exited, onClick, style }) => (
  <div
    onClick={onClick}
    style={{
      padding: 'var(--space-xs) var(--space-sm)',
      borderRadius: 'var(--radius-md)',
      cursor: onClick ? 'pointer' : 'default',
      background: active ? 'var(--bg-active)' : 'transparent',
      fontSize: 'var(--font-sm)',
      color: exited ? 'var(--text-muted)' : 'var(--text-primary)',
      display: 'flex',
      alignItems: 'center',
      gap: 'var(--space-sm)',
      transition: 'background 0.1s',
      ...style,
    }}
    onMouseEnter={e => {
      if (onClick && !active) {
        e.currentTarget.style.background = 'var(--bg-hover)';
      }
    }}
    onMouseLeave={e => {
      e.currentTarget.style.background = active ? 'var(--bg-active)' : 'transparent';
    }}
  >
    {children}
  </div>
);

export const FilterTabs: React.FC<{
  options: readonly string[];
  active: string;
  onChange: (value: string) => void;
  labels?: Record<string, string>;
}> = ({ options, active, onChange, labels }) => (
  <div style={{ display: 'flex', gap: 'var(--space-xs)', marginBottom: 'var(--space-md)' }}>
    {options.map(opt => {
      const isActive = active === opt;
      return (
        <button
          key={opt}
          onClick={() => onChange(opt)}
          style={{
            flex: 1,
            fontSize: 'var(--font-xs)',
            fontWeight: isActive ? 600 : 400,
            background: isActive ? 'var(--bg-active)' : 'var(--bg-hover)',
            border: '1px solid var(--border-default)',
            color: isActive ? 'var(--text-accent)' : 'var(--text-secondary)',
            cursor: 'pointer',
            padding: 'var(--space-xs) var(--space-sm)',
            borderRadius: 'var(--radius-md)',
            textTransform: 'capitalize',
            transition: 'background 0.12s, color 0.12s',
          }}
          onMouseEnter={e => {
            if (!isActive) {
              e.currentTarget.style.background = 'var(--bg-elevated)';
            }
          }}
          onMouseLeave={e => {
            if (!isActive) {
              e.currentTarget.style.background = 'var(--bg-hover)';
            }
          }}
        >
          {labels?.[opt] ?? opt}
        </button>
      );
    })}
  </div>
);

export const PopupMenu: React.FC<{
  children: React.ReactNode;
  style?: React.CSSProperties;
}> = ({ children, style }) => (
  <div
    style={{
      marginTop: 'var(--space-xs)',
      marginBottom: 'var(--space-xs)',
      background: 'var(--bg-elevated)',
      border: '1px solid var(--border-default)',
      borderRadius: 'var(--radius-md)',
      padding: 'var(--space-xs)',
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--space-xs)',
      boxShadow: '0 4px 16px rgba(0,0,0,0.4)',
      ...style,
    }}
  >
    {children}
  </div>
);

export const PopupMenuItem: React.FC<{
  onClick: React.MouseEventHandler<HTMLButtonElement>;
  children: React.ReactNode;
  shortcut?: string;
  danger?: boolean;
}> = ({ onClick, children, shortcut, danger }) => (
  <button
    onClick={onClick}
    style={{
      display: 'flex',
      alignItems: 'center',
      gap: 'var(--space-sm)',
      width: '100%',
      textAlign: 'left',
      fontSize: 'var(--font-sm)',
      background: 'transparent',
      border: 'none',
      color: danger ? 'var(--danger)' : 'var(--text-primary)',
      cursor: 'pointer',
      padding: 'var(--space-xs) var(--space-sm)',
      borderRadius: 'var(--radius-sm)',
      transition: 'background 0.1s',
    }}
    onMouseEnter={e => {
      e.currentTarget.style.background = 'var(--bg-hover)';
    }}
    onMouseLeave={e => {
      e.currentTarget.style.background = 'transparent';
    }}
  >
    <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
      {children}
    </span>
    {shortcut && (
      <span style={{ fontSize: 'var(--font-xs)', color: 'var(--text-accent)', minWidth: 40, textAlign: 'right' }}>
        {shortcut}
      </span>
    )}
  </button>
);
