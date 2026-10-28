import { fromJavaScript } from './dependency.js';

export function parseInput(value: string) {
  if (!value) throw new TypeError('Input is required');
  return value;
}

export function raise<T>(error: T): never {
  throw error;
}

export function wrapper() {
  parseInput('');
  fromJavaScript();
}

export function handled() {
  try { wrapper(); } catch { return 'handled'; }
}

export async function asyncFailure() {
  throw new RangeError('Out of range');
}

export async function awaited() {
  await asyncFailure();
}

parseInput('');
raise(new RangeError('Generic effect'));
wrapper();
handled();
asyncFailure();
awaited();
