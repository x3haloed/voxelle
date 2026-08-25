import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("./product-component.js", import.meta.url), "utf8");

test("attachment selection rejects unusable filenames before reading bytes", () => {
  const filenameCheck = source.indexOf("const filenameError = shortTextDraftError(file.name");
  const byteRead = source.indexOf("const data_b64 = await fileAsBase64(file)");
  assert(filenameCheck >= 0);
  assert(byteRead > filenameCheck);
  assert.match(source, /Rename the file, then choose it again\. Nothing was shared\./);
});

test("unusable browser MIME metadata degrades to the generic binary type", () => {
  assert.match(source, /const mimeError = shortTextDraftError\(selectedMime/);
  assert.match(source, /mime: mimeError \? "application\/octet-stream" : selectedMime/);
});
