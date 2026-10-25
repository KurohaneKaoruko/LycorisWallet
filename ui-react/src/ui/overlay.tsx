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
    /* 自动聚焦:仅限"宽屏 + 精确指针"环境。触屏聚焦输入框会立刻弹软键盘,把底部
       抽屉顶起再落下,与上滑动画叠加,观感是"内容从中间冒出来";窄屏抽屉同理
       (聚焦圈顶在视口下缘)。触屏/窄屏用户点输入框时再给焦点即可。 */
    const desktopish =
      window.matchMedia("(hover: hover) and (pointer: fine)").matches &&
      window.matchMedia("(min-width: 761px)").matches;
    const selector = desktopish ? "input, textarea, button.btn-primary" : "button.btn-primary";
    const first = backdropRef.current?.querySelector(selector);
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

/** Hidden-overflow hint for horizontally scrollable chip rows, plus the
 *  desktop affordances the old enhanceTabScroll grew: vertical wheel →
 *  horizontal scroll (releasing at the edges) and click-drag panning. */
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
      // 可滚动时给抓手光标，让桌面端知道这一行可以拖
      container.classList.toggle("is-scrollable", overflow > 4);
    };

    /* 桌面端滚轮：行内横向溢出时把纵向滚轮转成横向滚动；滚到一头后放行，
       让页面继续正常滚动。触控板的横向手势（deltaX 主导）交给原生处理。 */
    const onWheel = (event: WheelEvent) => {
      if (container.scrollWidth <= container.clientWidth + 1) return;
      if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
      const max = container.scrollWidth - container.clientWidth;
      const atStart = container.scrollLeft <= 0;
      const atEnd = container.scrollLeft >= max - 1;
      if ((event.deltaY < 0 && atStart) || (event.deltaY > 0 && atEnd)) return;
      event.preventDefault();
      container.scrollLeft += event.deltaY;
      update(); // 直接同步，不依赖 scroll 事件（后台标签页可能被节流）
    };

    /* 鼠标按住拖拽：位移超过阈值才算拖，松开后抑制那一次误点击。 */
    let dragging = false;
    let dragged = false;
    let startX = 0;
    let startLeft = 0;
    const onPointerDown = (event: PointerEvent) => {
      if (event.pointerType !== "mouse" || event.button !== 0) return;
      if (container.scrollWidth <= container.clientWidth + 1) return;
      dragging = true;
      dragged = false;
      startX = event.clientX;
      startLeft = container.scrollLeft;
    };
    const onPointerMove = (event: PointerEvent) => {
      if (!dragging) return;
      const delta = event.clientX - startX;
      if (!dragged && Math.abs(delta) > 5) {
        dragged = true;
        container.classList.add("is-dragging");
      }
      if (dragged) {
        container.scrollLeft = startLeft - delta;
        update(); // 直接同步，不依赖 scroll 事件（后台标签页可能被节流）
      }
    };
    const endDrag = () => {
      dragging = false;
      // 延迟清状态：click 在 pointerup 之后触发，需要让抑制器先看到 dragged。
      if (dragged) {
        window.setTimeout(() => {
          dragged = false;
          container.classList.remove("is-dragging");
        }, 0);
      } else {
        container.classList.remove("is-dragging");
      }
    };
    const onClickCapture = (event: MouseEvent) => {
      if (!dragged) return;
      event.preventDefault();
      event.stopPropagation();
    };

    container.addEventListener("scroll", update, { passive: true });
    container.addEventListener("wheel", onWheel, { passive: false });
    container.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("pointermove", onPointerMove, { passive: true });
    window.addEventListener("pointerup", endDrag, { passive: true });
    container.addEventListener("click", onClickCapture, true);
    const observer = new ResizeObserver(update);
    observer.observe(container);
    window.addEventListener("resize", update, { passive: true });
    update();
    return () => {
      container.removeEventListener("scroll", update);
      container.removeEventListener("wheel", onWheel);
      container.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", endDrag);
      container.removeEventListener("click", onClickCapture, true);
      window.removeEventListener("resize", update);
      observer.disconnect();
    };
  }, []);
  return ref;
}
