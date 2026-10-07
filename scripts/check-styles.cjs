const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const directory = path.resolve(__dirname, '../dist/filemelon/browser');
const html = fs.readFileSync(path.join(directory, 'index.html'), 'utf8');
const links = html.match(/<link\b[^>]*>/gi) || [];
const stylesheets = links.filter(link => /rel=["']stylesheet["']/i.test(link));
assert.ok(stylesheets.length, 'The production page must load global CSS.');
for (const link of stylesheets) {
  assert.ok(!/media=["']print["']/i.test(link), 'CSS must apply without running inline JavaScript.');
  const href = link.match(/href=["']([^"']+)["']/i)?.[1];
  assert.ok(href && fs.existsSync(path.join(directory, href)), 'The referenced stylesheet must exist.');
}
for (const script of html.match(/<script\b[^>]*>[\s\S]*?<\/script>/gi) || []) {
  assert.ok(/^<script\b[^>]*\bsrc=/i.test(script), 'Inline scripts are blocked by the desktop CSP.');
}
console.log('Production styles load directly and require no CSP-blocked inline scripts.');
