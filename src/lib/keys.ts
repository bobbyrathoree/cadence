interface KeyboardEventLike {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

const MODIFIER_KEYS = new Set(['Meta', 'Control', 'Shift', 'Alt']);

export function isModifierKey(key: string): boolean {
  return MODIFIER_KEYS.has(key);
}

export function normalizeKey(key: string): string {
  switch (key) {
    case ',':
      return 'Comma';
    case '.':
      return 'Period';
    case ' ':
      return 'Space';
    case 'ArrowUp':
      return 'Up';
    case 'ArrowDown':
      return 'Down';
    case 'ArrowLeft':
      return 'Left';
    case 'ArrowRight':
      return 'Right';
    default:
      return key.length === 1 ? key.toUpperCase() : key;
  }
}

export function eventToBinding(event: KeyboardEventLike): string {
  const parts: string[] = [];
  if (event.metaKey || event.ctrlKey) parts.push('CommandOrControl');
  if (event.shiftKey) parts.push('Shift');
  if (event.altKey) parts.push('Alt');
  if (!isModifierKey(event.key)) parts.push(normalizeKey(event.key));
  return parts.join('+');
}

export function shouldIgnoreShortcutFromTarget(
  target: EventTarget | null,
  event: Pick<
    KeyboardEventLike,
    'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey'
  >,
): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const isTextEntry =
    target.tagName === 'INPUT' ||
    target.tagName === 'TEXTAREA' ||
    target.isContentEditable;
  const hasModifier =
    event.metaKey || event.ctrlKey || event.shiftKey || event.altKey;
  return isTextEntry && !hasModifier;
}
