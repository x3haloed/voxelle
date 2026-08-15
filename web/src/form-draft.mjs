/**
 * Mirror the locally knowable shape of Rust's bounded human-facing names.
 * Rust remains authoritative when the command is admitted.
 * @param {string} value
 * @param {{ fieldName: string, emptyMessage: string, maxCharacters?: number }} options
 */
export function shortTextDraftError(value, options) {
  const maxCharacters = options.maxCharacters ?? 80;
  if (!value.trim()) return options.emptyMessage;
  if (value !== value.trim()) {
    return `${options.fieldName} cannot start or end with spaces.`;
  }
  if (/\p{Cc}/u.test(value)) {
    return `${options.fieldName} cannot contain control characters.`;
  }
  if ([...value].length > maxCharacters) {
    return `${options.fieldName} must be ${maxCharacters} characters or fewer.`;
  }
  return "";
}
