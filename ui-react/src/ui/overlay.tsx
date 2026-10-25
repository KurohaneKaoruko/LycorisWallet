/** Toast + modal primitives as React components (portal-based).
 *
 * Replaces the imperative openModal/toast API of ui/js/ui.js with a small
 * hook + provider pair so views can stay declarative. Class names match the
 * existing stylesheet (.toast, .modal-backdrop, .modal …) for zero CSS drift. */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import { Icon } from "./Icon";
import { copyText } from "../clipboard";

/* -------------------------------------------------------------------- toast */

export type ToastKind = "info" | "success" | "error";

interface ToastItem {
  id: number;
  message: string;
  kind: ToastKind;
  leaving: boolean;
}

const ToastContext = createContext<(message: string, kind?: ToastKind, timeout?: number) => void>(
  () => {},
);

export function useToast() {
  return useContext(ToastContext);
}

let toastSeq = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const timers = useRef(new Map<number, number>());

  const remove = useCallback((id: number) => {
    setItems((list) => list.map((t) => (t.id === id ? { ...t, leaving: true } : t)));
    window.setTimeout(() => setItems((list) => list.filter((t) => t.id !== id)), 220);
  }, []);

  const push = useCallback(
    (message: string, kind: ToastKind = "info", timeout = 3600) => {
      const id = ++toastSeq;
      setItems((list) => [...list, { id, message, kind, leaving: false }]);
      const timer = window.setTimeout(() => remove(id), timeout);
      timers.current.set(id, timer);
    },
    [remove],
  );

  useEffect(() => {
    const map = timers.current;
    return () => {
      for (const timer of map.values()) window.clearTimeout(timer);
      map.clear();
    };
  }, []);

  return (
    <ToastContext.Provider value={push}>
      {children}
      {createPortal(
        <div id="toasts">
          {items.map((item) => (
            <div
              key={item.id}
              className={`toast toast-${item.kind === "success" ? "ok" : item.kind}${item.leaving ? " out" : ""}`}
              onClick={() => {
                const timer = timers.current.get(item.id);
                if (timer) window.clearTimeout(timer);
                remove(item.id);
              }}
            >
              <Icon name={item.kind === "error" ? "warn" : item.kind === "success" ? "check" : "info"} />
              <div>{item.message}</div>
            </div>
          ))}
        </div>,
        document.body,
      )}
    </ToastContext.Provider>
  );
}

/* -------------------------------------------------------------------- modal */

export interface ModalProps {
  title?: string;
  description?: string;
  onClose: () => void;
  dismissable?: boolean;
  children: ReactNode;
}

export function Modal({ title, description, onClose, dismissable = true, children }: ModalProps) {
  const backdropRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && dismissable) onClose();
    };
    document.addEventListener("keydown", onKey);
    const first = backdropRef.current?.querySelector("input, textarea, button.btn-primary");
    (first as HTMLElement | null)?.focus();
    return () => document.removeEventListener("keydown", onKey);
  }, [dismissable, onClose]);

  return createPortal(
    <div
      ref={backdropRef}
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (dismissable && event.target === backdropRef.current) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true">
        {title ? <h2>{title}</h2> : null}
        {description ? <p className="hint" style={{ margin: "8px 0 14px" }}>{description}</p> : null}
        {children}
      </div>
    </div>,
    document.getElementById("modal-root") ?? document.body,
  );
}

/** Promise-flavoured helpers rendered through the shared ModalHost. */

interface ConfirmRequest {
  title: string;
  body: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
  resolve: (value: boolean) => void;
}

interface AskPasswordRequest {
  title: string;
  description?: string;
  confirmText?: string;
  resolve: (value: string | null) => void;
}

type ModalRequest = { kind: "confirm"; data: ConfirmRequest } | { kind: "ask"; data: AskPasswordRequest };

const ModalHostContext = createContext<{
  confirm: (req: Omit<ConfirmRequest, "resolve">) => Promise<boolean>;
  askPassword: (req: Omit<AskPasswordRequest, "resolve">) => Promise<string | null>;
}>({ confirm: () => Promise.resolve(false), askPassword: () => Promise.resolve(null) });

export function useDialogs() {
  return useContext(ModalHostContext);
}

export function ModalHostProvider({ children }: { children: ReactNode }) {
  const [requests, setRequests] = useState<ModalRequest[]>([]);

  const confirm = useCallback((req: Omit<ConfirmRequest, "resolve">) => {
    return new Promise<boolean>((resolve) => {
      setRequests((list) => [...list, { kind: "confirm", data: { ...req, resolve } }]);
    });
  }, []);

  const askPassword = useCallback((req: Omit<AskPasswordRequest, "resolve">) => {
    return new Promise<string | null>((resolve) => {
      setRequests((list) => [...list, { kind: "ask", data: { ...req, resolve } }]);
    });
  }, []);

  const settle = useCallback(
    (index: number, value: unknown) => {
      setRequests((list) => {
        const item = list[index];
        if (!item) return list;
        if (item.kind === "confirm") item.data.resolve(value as boolean);
        else item.data.resolve(value as string | null);
        return list.filter((_, i) => i !== index);
      });
    },
    [],
  );

  const value = useMemo(() => ({ confirm, askPassword }), [confirm, askPassword]);

  return (
    <ModalHostContext.Provider value={value}>
      {children}
      {requests.map((item, index) =>
        item.kind === "confirm" ? (
          <Modal
            key={index}
            title={item.data.title}
            onClose={() => settle(index, false)}
          >
            <div className="stack-sm" style={{ marginTop: 10 }} dangerouslySetInnerHTML={{ __html: item.data.body }} />
            <div className="modal-foot">
              <button className="btn btn-ghost" type="button" onClick={() => settle(index, false)}>
                {item.data.cancelText ?? "取消"}
              </button>
              <button
                className={`btn ${item.data.danger ? "btn-danger" : "btn-primary"}`}
                type="button"
                onClick={() => settle(index, true)}
              >
                {item.data.confirmText ?? "确认"}
              </button>
            </div>
          </Modal>
        ) : (
          <AskPasswordModal key={index} request={item.data} onDone={(value) => settle(index, value)} />
        ),
      )}
    </ModalHostContext.Provider>
  );
}

function AskPasswordModal({
  request,
  onDone,
}: {
  request: AskPasswordRequest;
  onDone: (value: string | null) => void;
}) {
  const [value, setValue] = useState("");
  const [error, setError] = useState("");

  const submit = () => {
    if (!value) {
      setError("请输入密码");
      return;
    }
    onDone(value);
  };

  return (
    <Modal title={request.title} description={request.description} onClose={() => onDone(null)}>
      <div className="field">
        <label htmlFor="ask-password">钱包密码</label>
        <input
          id="ask-password"
          className="input"
          type="password"
          autoComplete="current-password"
          placeholder="输入密码以继续"
          value={value}
          onChange={(event) => {
            setValue(event.target.value);
            setError("");
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") submit();
          }}
        />
      </div>
      {error ? <p className="error-text">{error}</p> : null}
      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={() => onDone(null)}>
          取消
        </button>
        <button className="btn btn-primary" type="button" onClick={submit}>
          {request.confirmText ?? "确认"}
        </button>
      </div>
    </Modal>
  );
}

/* -------------------------------------------------------------- copy widgets */

export function CopyField({ value, label }: { value: string; label?: string }) {
  const toast = useToast();
  return (
    <>
      <div className="copy-field">
        <span className="value mono">{value}</span>
        <button
          className="btn btn-ghost btn-sm"
          type="button"
          title="复制"
          aria-label="复制"
          onClick={async () => {
            const ok = await copyText(value);
            toast(ok ? "已复制" : "复制失败", ok ? "success" : "error");
          }}
        >
          <Icon name="copy" />
        </button>
      </div>
      {label ? <p className="hint">{label}</p> : null}
    </>
  );
}

/** Busy state helper — mirrors setBusy from ui/js/ui.js as a component. */
export function BusyButton({
  busy,
  busyText = "处理中…",
  children,
  className = "btn",
  ...rest
}: {
  busy: boolean;
  busyText?: string;
  children: ReactNode;
  className?: string;
} & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button className={className} disabled={busy || rest.disabled} {...rest}>
      {busy ? (
        <>
          <span className="spinner" />
          {busyText}
        </>
      ) : (
        children
      )}
    </button>
  );
}

/** Hidden-overflow hint for horizontally scrollable chip rows. */
export function useTabScroll<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    const container = ref.current;
    if (!container) return;
    const update = () => {
      const overflow = container.scrollWidth - container.clientWidth;
      const left = container.scrollLeft;
      container.classList.toggle("has-left", left > 4);
      container.classList.toggle("has-right", overflow > 4 && left < overflow - 4);
    };
    container.addEventListener("scroll", update, { passive: true });
    const observer = new ResizeObserver(update);
    observer.observe(container);
    update();
    return () => {
      container.removeEventListener("scroll", update);
      observer.disconnect();
    };
  }, []);
  return ref;
}
