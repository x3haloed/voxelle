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

/**
 * @param {string} value
 * @param {{ fieldName: string, maxCharacters: number }} options
 */
export function optionalTextDraftError(value, options) {
  if (/\p{Cc}/u.test(value)) {
    return `${options.fieldName} cannot contain control characters.`;
  }
  if ([...value].length > options.maxCharacters) {
    const maximum = options.maxCharacters.toLocaleString();
    return `${options.fieldName} must be ${maximum} characters or fewer.`;
  }
  return "";
}

/**
 * Mirror the locally knowable shape of Rust's retained-search query.
 * Leading and trailing whitespace remains valid because Rust trims it.
 * @param {string} value
 */
export function searchDraftError(value) {
  const query = value.trim();
  if (!query) return "Enter one or more words to search for.";
  if (/\p{Cc}/u.test(query)) return "Search terms cannot contain control characters.";
  if ([...query].length > 1024) return "Search terms must be 1,024 characters or fewer.";
  return "";
}
