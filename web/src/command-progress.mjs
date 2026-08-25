/**
 * Keep progress language tied to the semantic command definition so buttons,
 * shortcuts, the palette, and automation describe the same operation.
 * @param {string} commandId
 * @param {Array<{id: string, label: string}>} commands
 */
export function commandProgress(commandId, commands) {
  if (!commandId) return null;
  const label = commands.find((command) => command.id === commandId)?.label ?? commandId;
  return {
    buttonLabel: `${label}…`,
    announcement: `${label} is in progress. Voxelle will update this window when it finishes.`,
  };
}
