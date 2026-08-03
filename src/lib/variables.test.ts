import { describe, expect, it } from 'vitest';
import vectorsJson from '../../fixtures/variable-vectors.json?raw';
import {
  interpolate,
  parse,
  type Segment,
  variableNames,
} from './variables';

interface VariableVector {
  content: string;
  segments: Segment[];
  names: string[];
  interpolations: Array<{
    values: Record<string, string>;
    output: string;
  }>;
}

const vectors = JSON.parse(vectorsJson) as VariableVector[];

describe('canonical variable parser', () => {
  for (const [index, vector] of vectors.entries()) {
    it(`matches shared vector ${index + 1} byte-for-byte`, () => {
      const segments = parse(vector.content);
      expect(segments).toEqual(vector.segments);
      expect(variableNames(segments)).toEqual(vector.names);
      expect(segments.map((segment) => segment.raw).join('')).toBe(
        vector.content,
      );
      for (const interpolation of vector.interpolations) {
        expect(
          interpolate(
            segments,
            new Map(Object.entries(interpolation.values)),
          ),
        ).toBe(interpolation.output);
      }
    });
  }

  it('preserves every input byte across generated inputs', () => {
    let state = 0x5eed1234;
    const alphabet = [
      '{',
      '}',
      '[',
      ']',
      'A',
      'a',
      ' ',
      '\t',
      '\n',
      '\r',
      '_',
      '🚀',
    ];
    for (let sample = 0; sample < 1_000; sample += 1) {
      let content = '';
      const length = sample % 41;
      for (let index = 0; index < length; index += 1) {
        state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
        content += alphabet[state % alphabet.length];
      }
      const segments = parse(content);
      expect(segments.every((segment) => segment.raw.length > 0)).toBe(true);
      expect(segments.map((segment) => segment.raw).join('')).toBe(content);
    }
  });
});
