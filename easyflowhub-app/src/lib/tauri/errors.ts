export interface NativeErrorMessages {
  unavailable: string;
  fallback: string;
}

function rawErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  if (error && typeof error === 'object' && 'message' in error) return String(error.message);
  return '';
}

function isBrowserBridgeError(message: string): boolean {
  return /cannot read properties of (undefined|null) \(reading ['"]invoke['"]\)/i.test(message)
    || /(?:invoke|tauri).*(?:is not a function|is undefined|unavailable|not available)/i.test(message);
}

/** Normalize browser-preview bridge failures while preserving native errors. */
export function formatNativeError(error: unknown, messages: NativeErrorMessages): string {
  const message = rawErrorMessage(error);
  if (isBrowserBridgeError(message)) return messages.unavailable;
  return message || messages.fallback;
}
