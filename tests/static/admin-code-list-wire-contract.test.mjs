#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const adminRepositorySource = readFileSync(
  new URL(
    '../../crates/sdkwork-commerce-promotion-repository-sqlx/src/promotion_admin.rs',
    import.meta.url,
  ),
  'utf8',
);

test('admin code list returns the full promotion code on the wire', () => {
  // The admin console masks the code for display, but its copy and CSV export
  // actions consume this field. Masking it here makes "copy full code" copy the
  // masked text and exports masked codes, so the wire value stays plaintext.
  assert.match(
    adminRepositorySource,
    /promotion_code: row\.string\("promotion_code"\)\?/,
    'list_codes must map the stored promotion_code without masking it',
  );
  assert.doesNotMatch(
    adminRepositorySource,
    /mask_code/,
    'the admin code list must not mask promotion_code on the wire',
  );
});
