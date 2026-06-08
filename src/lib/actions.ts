/**
 * Focus the node on mount when `enabled` (default true).
 *
 * Use instead of the `autofocus` attribute, which trips Svelte's
 * `a11y_autofocus` warning. Autofocus is appropriate here because these
 * are modal dialogs where focusing the primary input is the expected,
 * keyboard-friendly behaviour.
 */
export function autofocus(node: HTMLElement, enabled = true) {
  if (enabled) node.focus();
}
