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

/**
 * Advisory mirror for the bracketed IPv6 socket shape accepted by Rust.
 * Empty values remain valid because ordinary startup chooses local defaults.
 * @param {string} value
 * @param {string} fieldName
 */
export function optionalIpv6SocketDraftError(value, fieldName) {
  if (!value) return "";
  if (value !== value.trim()) return `${fieldName} cannot start or end with spaces.`;
  if (/\p{Cc}/u.test(value)) return `${fieldName} cannot contain control characters.`;
  const socket = value.match(/^\[([^\]]+)\]:(\d+)$/u);
  if (!socket) return `${fieldName} must use a bracketed IPv6 address and port, such as [::]:0.`;
  const port = Number(socket[2]);
  if (!Number.isSafeInteger(port) || port > 65535) {
    return `${fieldName} port must be between 0 and 65,535.`;
  }
  if (!isIpv6Address(socket[1])) return `${fieldName} contains an invalid IPv6 address.`;
  return "";
}

function isIpv6Address(value) {
  const zoneParts = value.split("%");
  if (zoneParts.length > 2 || (zoneParts.length === 2 && !/^\d+$/u.test(zoneParts[1]))) return false;
  let address = zoneParts[0];
  const groups = address.split(":");
  const dottedIndex = groups.findIndex((group) => group.includes("."));
  if (dottedIndex >= 0) {
    if (dottedIndex !== groups.length - 1 || !isIpv4Tail(groups[dottedIndex])) return false;
    groups.splice(dottedIndex, 1, "0", "0");
    address = groups.join(":");
  }
  if (!/^[0-9a-f:]+$/iu.test(address)) return false;
  const compressed = address.includes("::");
  if (compressed && address.indexOf("::") !== address.lastIndexOf("::")) return false;
  const populated = address.split(":").filter(Boolean);
  if (!populated.every((group) => /^[0-9a-f]{1,4}$/iu.test(group))) return false;
  return compressed ? populated.length < 8 : populated.length === 8;
}

function isIpv4Tail(value) {
  const octets = value.split(".");
  return octets.length === 4
    && octets.every((octet) => (
      /^\d{1,3}$/u.test(octet)
      && octet === String(Number(octet))
      && Number(octet) <= 255
    ));
}
