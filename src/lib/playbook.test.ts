import { describe, expect, it } from 'vitest';
import { getPlaybookProgress } from './playbook';

describe('getPlaybookProgress', () => {
  it('uses the actual total step count and clamps the result', () => {
    expect(getPlaybookProgress(1, 5)).toBe(0.2);
    expect(getPlaybookProgress(2, 8)).toBe(0.25);
    expect(getPlaybookProgress(-1, 4)).toBe(0);
    expect(getPlaybookProgress(5, 3)).toBe(1);
    expect(getPlaybookProgress(0, 0)).toBe(0);
  });
});
