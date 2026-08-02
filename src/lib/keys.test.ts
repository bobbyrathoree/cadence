import { describe, expect, it } from 'vitest';
import {
  eventToBinding,
  normalizeKey,
  shouldIgnoreShortcutFromTarget,
} from './keys';

describe('keyboard shortcut normalization', () => {
  it('uses canonical Tauri arrow key names', () => {
    expect(normalizeKey('ArrowUp')).toBe('Up');
    expect(normalizeKey('ArrowDown')).toBe('Down');
    expect(normalizeKey('ArrowLeft')).toBe('Left');
    expect(normalizeKey('ArrowRight')).toBe('Right');
  });

  it('normalizes punctuation, spaces, and character casing', () => {
    expect(normalizeKey(',')).toBe('Comma');
    expect(normalizeKey('.')).toBe('Period');
    expect(normalizeKey(' ')).toBe('Space');
    expect(normalizeKey('p')).toBe('P');
  });

  it('builds one canonical binding for application and recorder use', () => {
    expect(
      eventToBinding({
        key: 'ArrowDown',
        metaKey: true,
        ctrlKey: false,
        shiftKey: true,
        altKey: false,
      }),
    ).toBe('CommandOrControl+Shift+Down');
  });
});

describe('shouldIgnoreShortcutFromTarget', () => {
  it('ignores unmodified typing but allows modifier shortcuts in text fields', () => {
    const input = document.createElement('input');
    expect(
      shouldIgnoreShortcutFromTarget(input, {
        metaKey: false,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
      }),
    ).toBe(true);
    expect(
      shouldIgnoreShortcutFromTarget(input, {
        metaKey: true,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
      }),
    ).toBe(false);
  });
});
