import test from "node:test";
import assert from "node:assert/strict";
import {
  productConfirmationContent,
  productConfirmationRequired,
} from "./product-update-confirmation.mjs";

const generation = {
  active_release_id: "v0.1.0-beta.2",
  staged_release_id: "v0.1.0-beta.3",
};

test("every consequential product command requires human confirmation", () => {
  for (const command of [
    "product.update.install",
    "product.update.activateStaged",
    "product.update.rollback",
    "product.update.rotateTrust",
  ]) {
    assert.equal(productConfirmationRequired(command), true, command);
    assert.ok(productConfirmationContent(command, generation), command);
  }
  assert.equal(productConfirmationRequired("product.update.check"), false);
  assert.equal(productConfirmationRequired("product.update.stageAvailable"), false);
  assert.equal(productConfirmationRequired("product.update.discardStaged"), false);
});

test("activation and trust review name the authority-relevant consequences", () => {
  assert.match(
    productConfirmationContent("product.update.activateStaged", generation).description,
    /v0\.1\.0-beta\.2/,
  );
  assert.match(
    productConfirmationContent("product.update.activateStaged", generation).title,
    /v0\.1\.0-beta\.3/,
  );
  assert.match(
    productConfirmationContent("product.update.rotateTrust", generation).description,
    /which release keys may authorize future product generations/,
  );
});
