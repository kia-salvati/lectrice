import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { toApiError } from '../src/api/errorResponse.ts';
import { CLIENT_ERROR_NAMES, ERROR_CODES } from '../src/config/errorRegistry.ts';
import { hasDesktopErrorName } from '../src/desktop/commands/errors.ts';
import { toDesktopError } from '../src/desktop/commands/wire.ts';

/** Set it to check the mirror against another checkout of the core (e.g. a worktree that is ahead of this one). */
const ERROR_RS_ENV = 'LECTRICE_ERROR_RS';
const DEFAULT_ERROR_RS = fileURLToPath(new URL('../../backend/crates/lectrice-domain/src/error.rs', import.meta.url));
const errorRsPath = process.env[ERROR_RS_ENV] ?? DEFAULT_ERROR_RS;

/** The `Name => (code, …)` lines inside `error_registry! { … }`. */
const readRustRegistry = (source: string): Map<string, number> => {
  const block = /error_registry!\s*\{([\s\S]*?)\n\}/.exec(source);
  assert.ok(block, 'error_registry! block not found');
  return new Map([...block[1].matchAll(/^\s*(\w+)\s*=>\s*\((\d+),/gm)].map((match) => [match[1], Number(match[2])]));
};

test('every core error name is mirrored with its code', { skip: !existsSync(errorRsPath) && `${errorRsPath} not found` }, () => {
  const rust = readRustRegistry(readFileSync(errorRsPath, 'utf8'));
  assert.ok(rust.size > 0);
  const mirror: Record<string, number> = ERROR_CODES;
  for (const [name, code] of rust) assert.equal(mirror[name], code, `${name} should be ${code}`);
});

// This checkout's own core can lag behind the core release the UI is built for, so only a named file is held to it.
test('the mirror has no name the core lacks', (t) => {
  if (!existsSync(errorRsPath)) return t.skip(`${errorRsPath} not found`);
  const rust = readRustRegistry(readFileSync(errorRsPath, 'utf8'));
  const extra = Object.keys(ERROR_CODES).filter((name) => !rust.has(name));
  if (extra.length > 0 && process.env[ERROR_RS_ENV] === undefined) return t.skip(`this checkout's core lacks ${extra.join(', ')}; set ${ERROR_RS_ENV} to check a newer core`);
  assert.deepEqual(extra, []);
});

test('client error names never clash with the registry', () => {
  for (const name of CLIENT_ERROR_NAMES) assert.ok(!Object.hasOwn(ERROR_CODES, name), name);
});

const jsonResponse = (status: number, body: unknown): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

test('a server error keeps its name, code, message and status', async () => {
  const error = await toApiError(jsonResponse(422, { error: { name: 'PdfTooComplex', code: 1010, message: 'This PDF is too long or too complex to open.' } }));
  assert.equal(error.name, 'PdfTooComplex');
  assert.equal(error.code, 1010);
  assert.equal(error.message, 'This PDF is too long or too complex to open.');
  assert.equal(error.status, 422);
});

test('an error name this build does not know keeps the server message and code', async () => {
  const error = await toApiError(jsonResponse(409, { error: { name: 'SomethingNewer', code: 7001, message: 'A newer server says why.' } }));
  assert.equal(error.name, 'Internal');
  assert.equal(error.code, 7001);
  assert.equal(error.message, 'A newer server says why.');
  assert.equal(error.status, 409);
});

test('an answer without an error body becomes the generic internal error', async () => {
  for (const response of [new Response('<html>', { status: 502 }), jsonResponse(500, { error: { name: 'Internal' } })]) {
    const error = await toApiError(response);
    assert.equal(error.name, 'Internal');
    assert.equal(error.code, ERROR_CODES.Internal);
    assert.equal(error.message, 'Something went wrong.');
    assert.equal(error.status, response.status);
  }
});

test('a desktop rejection with an unknown name keeps its message and code', () => {
  const error = toDesktopError({ error: { name: 'SomethingNewer', code: 7001, message: 'A newer shell says why.' } });
  assert.ok(hasDesktopErrorName(error, 'Internal'));
  assert.equal(error.code, 7001);
  assert.equal(error.message, 'A newer shell says why.');
  assert.ok(hasDesktopErrorName(toDesktopError({ error: { name: 'HelperBusy', code: 5006, message: 'Busy.' } }), 'HelperBusy'));
});
