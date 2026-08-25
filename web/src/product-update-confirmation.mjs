const CONFIRMED_PRODUCT_COMMANDS = new Set([
  "product.update.install",
  "product.update.activateStaged",
  "product.update.rollback",
  "product.update.rotateTrust",
]);

export function productConfirmationRequired(command) {
  return CONFIRMED_PRODUCT_COMMANDS.has(command);
}

export function productConfirmationContent(command, generation) {
  return {
    "product.update.install": {
      title: "Install this signed product package?",
      description: "The native kernel will authenticate, stage, and activate the selected package. An invalid, untrusted, downgraded, or incompatible package will be rejected without changing the active generation.",
      confirm: "Verify and install package",
    },
    "product.update.activateStaged": {
      title: `Activate ${generation.staged_release_id || "the staged generation"}?`,
      description: `Voxelle will switch the running product surface from ${generation.active_release_id} to the verified staged generation. The previous verified generation remains available for rollback.`,
      confirm: "Activate staged update",
    },
    "product.update.rollback": {
      title: "Roll back the product surface?",
      description: `Voxelle will leave the native kernel and retained protocol state in place while reactivating the previous verified product generation instead of ${generation.active_release_id}.`,
      confirm: "Roll back product update",
    },
    "product.update.rotateTrust": {
      title: "Change release-signing trust?",
      description: "This changes which release keys may authorize future product generations. The native kernel requires an ordered transition signed by a currently trusted key; GitHub, mirrors, and the transition payload cannot authorize themselves.",
      confirm: "Apply signed trust transition",
    },
  }[command] ?? null;
}
