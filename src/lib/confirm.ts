export type ConfirmMessagePart = {
  text: string;
  emph?: boolean;
};

export type ConfirmMessageBody = {
  primary: ConfirmMessagePart[];
  secondary?: ConfirmMessagePart[];
};

export type ConfirmRequest = {
  title: string;
  message?: string;
  messageBody?: ConfirmMessageBody;
  confirmLabel?: string;
  cancelLabel?: string;
  /** When false (default), confirm uses neutral styling — user already chose a deliberate action. */
  destructiveConfirm?: boolean;
};

function joinConfirmParts(parts: ConfirmMessagePart[]): string {
  return parts.map((part) => part.text).join("");
}

export function flattenConfirmMessage(body: ConfirmMessageBody): string {
  const primary = joinConfirmParts(body.primary);
  return body.secondary ? `${primary} ${joinConfirmParts(body.secondary)}` : primary;
}

type Resolver = (confirmed: boolean) => void;

type PendingConfirm = {
  request: ConfirmRequest;
  resolve: Resolver;
};

let activeRequest: ConfirmRequest | null = null;
let activeResolver: Resolver | null = null;
let returnFocusElement: HTMLElement | null = null;
const queue: PendingConfirm[] = [];
const listeners = new Set<() => void>();

function notify() {
  for (const listener of listeners) listener();
}

function captureReturnFocus(): void {
  returnFocusElement =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
}

function restoreReturnFocus(): void {
  const focusEl = returnFocusElement;
  returnFocusElement = null;
  if (focusEl?.isConnected) {
    queueMicrotask(() => focusEl.focus());
  }
}

function showNextConfirm(): void {
  if (activeRequest || queue.length === 0) return;

  const next = queue.shift();
  if (!next) return;

  captureReturnFocus();
  activeRequest = {
    confirmLabel: "OK",
    cancelLabel: "Cancel",
    destructiveConfirm: false,
    ...next.request,
  };
  activeResolver = next.resolve;
  notify();
}

export function subscribeConfirm(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getConfirmRequest(): ConfirmRequest | null {
  return activeRequest;
}

export function confirmDestructive(request: ConfirmRequest): Promise<boolean> {
  return new Promise((resolve) => {
    queue.push({ request, resolve });
    showNextConfirm();
  });
}

export function resolveConfirm(confirmed: boolean): void {
  const resolver = activeResolver;
  activeRequest = null;
  activeResolver = null;
  notify();
  resolver?.(confirmed);
  restoreReturnFocus();
  showNextConfirm();
}
