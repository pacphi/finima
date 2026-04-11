import { useEffect, useRef, useCallback } from 'react';

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  '[role="button"]:not([disabled])',
].join(', ');

/**
 * Traps keyboard focus within a container element while active.
 * Returns a ref to attach to the container element.
 *
 * Features:
 * - Traps Tab / Shift+Tab within the container
 * - Calls onEscape when Escape is pressed
 * - Restores focus to the previously focused element on deactivation
 * - Auto-focuses the first focusable element on activation
 */
export function useFocusTrap<T extends HTMLElement = HTMLDivElement>(
  isActive: boolean,
  onEscape?: () => void,
) {
  const containerRef = useRef<T>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  const getFocusableElements = useCallback((): HTMLElement[] => {
    if (!containerRef.current) return [];
    return Array.from(
      containerRef.current.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
    ).filter(
      (el) =>
        !el.hasAttribute('disabled') &&
        el.getAttribute('tabindex') !== '-1' &&
        el.offsetParent !== null,
    );
  }, []);

  useEffect(() => {
    if (!isActive) return;

    // Store the currently focused element so we can restore it later.
    previousFocusRef.current = document.activeElement as HTMLElement | null;

    // Move focus into the trap after a micro-task to ensure the DOM has rendered.
    const frameId = requestAnimationFrame(() => {
      const focusable = getFocusableElements();
      if (focusable.length > 0 && focusable[0]) {
        focusable[0].focus();
      } else {
        // If no focusable children, focus the container itself.
        containerRef.current?.focus();
      }
    });

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && onEscape) {
        event.stopPropagation();
        onEscape();
        return;
      }

      if (event.key !== 'Tab') return;

      const focusable = getFocusableElements();
      if (focusable.length === 0) {
        event.preventDefault();
        return;
      }

      const first = focusable[0]!;
      const last = focusable[focusable.length - 1]!;

      if (event.shiftKey) {
        if (document.activeElement === first) {
          event.preventDefault();
          last.focus();
        }
      } else {
        if (document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);

    return () => {
      cancelAnimationFrame(frameId);
      document.removeEventListener('keydown', handleKeyDown);

      // Restore focus to the element that was focused before the trap activated.
      if (previousFocusRef.current && typeof previousFocusRef.current.focus === 'function') {
        previousFocusRef.current.focus();
      }
    };
  }, [isActive, onEscape, getFocusableElements]);

  return containerRef;
}
