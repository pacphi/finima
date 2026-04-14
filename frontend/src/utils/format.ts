import { usePrefsStore } from '../stores/prefsStore';

/**
 * Format a number as currency using the user's preferred currency from prefsStore.
 * Falls back to USD if no preference is set.
 */
export function formatCurrency(amount: number): string {
  const { currency } = usePrefsStore.getState();
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: currency || 'USD',
  }).format(amount);
}

/**
 * Format a number as currency with no fractional digits.
 * Useful for summary displays where cents are not needed.
 */
export function formatCurrencyCompact(amount: number): string {
  const { currency } = usePrefsStore.getState();
  const value = typeof amount === 'number' && !isNaN(amount) ? amount : 0;
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: currency || 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

/**
 * Map the user's dateFormat preference string to Intl.DateTimeFormat options.
 */
function getDateOptions(dateFormat: string): Intl.DateTimeFormatOptions {
  switch (dateFormat) {
    case 'DD/MM/YYYY':
      return { day: '2-digit', month: '2-digit', year: 'numeric' };
    case 'YYYY-MM-DD':
      return { year: 'numeric', month: '2-digit', day: '2-digit' };
    case 'MM/DD/YYYY':
    default:
      return { month: '2-digit', day: '2-digit', year: 'numeric' };
  }
}

/**
 * Format a date string or Date object using the user's preferred date format.
 */
export function formatDate(date: string | Date): string {
  const { dateFormat } = usePrefsStore.getState();
  const d = typeof date === 'string' ? new Date(date) : date;

  // For YYYY-MM-DD we use a locale that naturally produces that format
  if (dateFormat === 'YYYY-MM-DD') {
    return d.toLocaleDateString('sv-SE', getDateOptions(dateFormat));
  }

  // For DD/MM/YYYY use en-GB which naturally produces dd/mm/yyyy
  if (dateFormat === 'DD/MM/YYYY') {
    return d.toLocaleDateString('en-GB', getDateOptions(dateFormat));
  }

  // Default MM/DD/YYYY
  return d.toLocaleDateString('en-US', getDateOptions(dateFormat));
}
