export type Segment =
  | { kind: 'text'; raw: string }
  | { kind: 'variable'; raw: string; name: string }
  | { kind: 'placeholder'; raw: string };

function trimVariableName(value: string): string {
  let start = 0;
  let end = value.length;
  while (start < end && (value[start] === ' ' || value[start] === '\t')) {
    start += 1;
  }
  while (end > start && (value[end - 1] === ' ' || value[end - 1] === '\t')) {
    end -= 1;
  }
  return value.slice(start, end);
}

function variableAt(content: string, index: number): Segment | null {
  if (!content.startsWith('{{', index)) return null;

  let close = content.indexOf('}}', index + 2);
  while (close !== -1) {
    const interior = content.slice(index + 2, close);
    if (!/[{}\n\r]/.test(interior)) {
      const name = trimVariableName(interior);
      if (name.length > 0) {
        return {
          kind: 'variable',
          raw: content.slice(index, close + 2),
          name,
        };
      }
      return null;
    }
    close = content.indexOf('}}', close + 2);
  }
  return null;
}

function placeholderAt(content: string, index: number): Segment | null {
  if (content[index] !== '[') return null;
  const match = /^\[[A-Z][A-Z _]*\]/.exec(content.slice(index));
  return match ? { kind: 'placeholder', raw: match[0] } : null;
}

export function parse(content: string): Segment[] {
  const segments: Segment[] = [];
  let index = 0;
  let textStart = 0;

  function flushText(end: number) {
    if (end > textStart) {
      segments.push({ kind: 'text', raw: content.slice(textStart, end) });
    }
  }

  while (index < content.length) {
    const variable = variableAt(content, index);
    const placeholder = variable ? null : placeholderAt(content, index);
    const token = variable ?? placeholder;
    if (!token) {
      index += 1;
      continue;
    }

    flushText(index);
    segments.push(token);
    index += token.raw.length;
    textStart = index;
  }
  flushText(content.length);
  return segments;
}

export function variableNames(segments: Segment[]): string[] {
  const seen = new Set<string>();
  const names: string[] = [];
  for (const segment of segments) {
    if (segment.kind === 'variable' && !seen.has(segment.name)) {
      seen.add(segment.name);
      names.push(segment.name);
    }
  }
  return names;
}

export function interpolate(
  segments: Segment[],
  values: Map<string, string>,
): string {
  return segments
    .map((segment) => {
      if (segment.kind !== 'variable') return segment.raw;
      const value = values.get(segment.name);
      return value ? value : segment.raw;
    })
    .join('');
}
