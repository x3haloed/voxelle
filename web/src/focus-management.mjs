const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "summary",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

/** @param {ParentNode} container */
export function focusableElements(container) {
  return [...container.querySelectorAll(FOCUSABLE_SELECTOR)].filter((element) =>
    element.getAttribute("aria-hidden") !== "true"
    && !element.closest?.("[hidden], [inert]")
    && visibleWithinDetails(element)
    && (typeof element.getClientRects !== "function" || element.getClientRects().length > 0));
}

function visibleWithinDetails(element) {
  const closedDetails = element.closest?.("details:not([open])");
  return !closedDetails
    || (element.tagName === "SUMMARY" && element.parentElement === closedDetails);
}

/**
 * Keep keyboard navigation inside a modal surface.
 * @param {KeyboardEvent} event
 * @param {HTMLElement} container
 * @returns {boolean} whether the event was handled
 */
export function trapModalTab(event, container) {
  if (event.key !== "Tab") return false;
  const focusable = focusableElements(container);
  const first = focusable[0] ?? container;
  const last = focusable.at(-1) ?? container;
  const active = container.ownerDocument.activeElement;
  const leavingBackward = event.shiftKey && (active === first || !container.contains(active));
  const leavingForward = !event.shiftKey && (active === last || !container.contains(active));
  if (!leavingBackward && !leavingForward) return false;
  event.preventDefault();
  (leavingBackward ? last : first).focus();
  return true;
}

export class FocusSurfaceCoordinator {
  /** @param {Document} document @param {(callback: FrameRequestCallback) => number} schedule */
  constructor(document, schedule) {
    this.document = document;
    this.schedule = schedule;
    this.returnElement = null;
    this.surface = "";
  }

  rememberReturnElement() {
    if (this.document.activeElement?.focus) {
      this.returnElement = this.document.activeElement;
    }
  }

  currentElement() {
    const active = this.document.activeElement;
    if (
      !active?.focus
      || active === this.document.body
      || active === this.document.documentElement
    ) return null;
    return active;
  }

  /** @param {HTMLElement | null} target @param {() => HTMLElement | null} [fallback] */
  restoreWhenNoSurface(target, fallback = () => null) {
    this.schedule(() => {
      if (this.surface) return;
      if (target?.isConnected) {
        target.focus();
      } else {
        fallback()?.focus();
      }
    });
  }

  /** @param {string} surface @param {() => HTMLElement | null} initialTarget */
  synchronize(surface, initialTarget) {
    if (surface === this.surface) return;
    const previousSurface = this.surface;
    this.surface = surface;
    if (surface) {
      this.schedule(() => {
        if (this.surface === surface) initialTarget()?.focus();
      });
    } else if (previousSurface && this.returnElement) {
      const target = this.returnElement;
      this.schedule(() => {
        if (!this.surface && target.isConnected) {
          target.focus();
          this.returnElement = null;
        }
      });
    }
  }
}
