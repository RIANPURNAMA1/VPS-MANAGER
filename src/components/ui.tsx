import { forwardRef, useEffect, useId, useRef, useState } from "react";
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";
import { AlertTriangle, CheckCircle2, Circle, Inbox, Loader2, XCircle } from "lucide-react";
import { createPortal } from "react-dom";

export const cx = (...a: Array<string | false | null | undefined>) => a.filter(Boolean).join(" ");

/* ---------- Button ---------- */
type BtnProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" | "danger"; size?: "sm" | "md"; loading?: boolean };
export const Button = forwardRef<HTMLButtonElement, BtnProps>(function Button(
  { variant = "secondary", size = "md", loading, className, children, disabled, type = "button", ...rest }, ref,
) {
  const v = {
    primary: "bg-btn text-btnfg border-btn hover:opacity-85",
    secondary: "bg-bg text-fg border-line hover:bg-hover",
    ghost: "bg-transparent text-fg border-transparent hover:bg-hover",
    // "danger" tetap monokrom: dibedakan dengan garis putus-putus + ikon pada pemanggil
    danger: "bg-bg text-fg border-fg border-dashed hover:bg-hover",
  }[variant];
  return (
    <button
      ref={ref}
      type={type}
      disabled={disabled || loading}
      className={cx(
        "inline-flex items-center justify-center gap-2 rounded-md border font-medium whitespace-nowrap transition-colors disabled:opacity-45 disabled:cursor-not-allowed",
        size === "sm" ? "h-7 px-2.5 text-[13px]" : "h-9 px-3.5",
        v, className,
      )}
      {...rest}
    >
      {loading && <Loader2 className="size-4 animate-spin" aria-hidden />}
      {children}
    </button>
  );
});

export function IconButton({ label, children, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <Tooltip text={label}>
      <button type="button" aria-label={label} className="inline-flex size-8 items-center justify-center rounded-md text-fg2 hover:bg-hover hover:text-fg disabled:opacity-45" {...rest}>
        {children}
      </button>
    </Tooltip>
  );
}

export function Tooltip({ text, children }: { text: string; children: ReactNode }) {
  return (
    <span className="group relative inline-flex">
      {children}
      <span role="tooltip" className="pointer-events-none absolute left-1/2 top-full z-40 mt-1 hidden -translate-x-1/2 whitespace-nowrap rounded-md border border-line bg-fg px-2 py-1 text-xs text-bg group-hover:block group-focus-within:block">
        {text}
      </span>
    </span>
  );
}

/* ---------- Form ---------- */
const field = "h-9 w-full rounded-md border border-line bg-bg px-3 text-fg placeholder:text-fg3 disabled:opacity-50";
export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input({ className, ...p }, ref) {
  return <input ref={ref} className={cx(field, className)} {...p} />;
});
export const Select = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(function Select({ className, ...p }, ref) {
  return <select ref={ref} className={cx(field, "pr-8", className)} {...p} />;
});
export function Textarea({ className, ...p }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={cx("w-full rounded-md border border-line bg-bg p-3 text-fg placeholder:text-fg3", className)} {...p} />;
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: (id: string) => ReactNode }) {
  const id = useId();
  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="block text-[13px] font-medium">{label}</label>
      {children(id)}
      {hint && <p className="text-xs text-fg2">{hint}</p>}
    </div>
  );
}

export function Checkbox({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode; disabled?: boolean }) {
  return (
    <label className={cx("inline-flex items-center gap-2", disabled && "opacity-50")}>
      <input type="checkbox" className="size-4 accent-[var(--fg)]" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

/* ---------- Layout ---------- */
export function Card({ title, actions, children, className, pad = true }: { title?: ReactNode; actions?: ReactNode; children: ReactNode; className?: string; pad?: boolean }) {
  return (
    <section className={cx("rounded-md border border-line bg-card", className)}>
      {(title || actions) && (
        <header className="flex min-h-12 items-center justify-between gap-3 border-b border-line px-4 py-2">
          <h2 className="text-[14px] font-semibold">{title}</h2>
          <div className="flex items-center gap-2">{actions}</div>
        </header>
      )}
      <div className={pad ? "p-4" : ""}>{children}</div>
    </section>
  );
}

export function PageHeader({ title, description, actions }: { title: string; description?: string; actions?: ReactNode }) {
  return (
    <div className="mb-5 flex items-start justify-between gap-4">
      <div>
        <h1 className="text-xl font-semibold">{title}</h1>
        {description && <p className="mt-0.5 max-w-2xl text-fg2">{description}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{actions}</div>
    </div>
  );
}

/* ---------- Status ---------- */
export type Tone = "ok" | "error" | "warn" | "neutral" | "active";
const toneIcon: Record<Tone, ReactNode> = {
  ok: <CheckCircle2 className="size-3.5" aria-hidden />,
  error: <XCircle className="size-3.5" aria-hidden />,
  warn: <AlertTriangle className="size-3.5" aria-hidden />,
  neutral: <Circle className="size-3.5" aria-hidden />,
  active: <Loader2 className="size-3.5 animate-spin" aria-hidden />,
};
/** Status dibedakan lewat ikon, label, dan gaya garis — bukan warna. */
const toneStyle: Record<Tone, string> = {
  ok: "border-fg border-solid text-fg",
  error: "border-fg border-dashed text-fg font-semibold",
  warn: "border-fg2 border-dotted text-fg",
  neutral: "border-line border-solid text-fg2",
  active: "border-fg bg-fg text-bg",
};
export function Badge({ tone = "neutral", children, className }: { tone?: Tone; children: ReactNode; className?: string }) {
  return (
    <span className={cx("inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-xs", toneStyle[tone], className)}>
      {toneIcon[tone]}
      {children}
    </span>
  );
}

export function Spinner({ label = "Memuat…" }: { label?: string }) {
  return (
    <div role="status" className="flex items-center justify-center gap-2 py-10 text-fg2">
      <Loader2 className="size-4 animate-spin" aria-hidden /> {label}
    </div>
  );
}

export function EmptyState({ icon, title, description, action }: { icon?: ReactNode; title: string; description?: string; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 px-6 py-12 text-center">
      <div className="text-fg3">{icon ?? <Inbox className="size-8" aria-hidden />}</div>
      <p className="font-medium">{title}</p>
      {description && <p className="max-w-md text-fg2">{description}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

export function ErrorState({ message, onRetry, title = "Terjadi kesalahan" }: { message: string; onRetry?: () => void; title?: string }) {
  return (
    <div role="alert" className="m-4 rounded-md border border-dashed border-fg p-4">
      <p className="flex items-center gap-2 font-semibold"><XCircle className="size-4" aria-hidden /> {title}</p>
      <p className="mt-1 whitespace-pre-wrap break-words text-fg2">{message}</p>
      {onRetry && <Button size="sm" className="mt-3" onClick={onRetry}>Coba lagi</Button>}
    </div>
  );
}

export function Progress({ value, label }: { value: number; label?: string }) {
  const v = Math.max(0, Math.min(100, value));
  return (
    <div role="progressbar" aria-valuenow={Math.round(v)} aria-valuemin={0} aria-valuemax={100} aria-label={label} className="h-2 w-full overflow-hidden rounded-md border border-line bg-bg">
      <div className="h-full bg-fg transition-[width]" style={{ width: `${v}%` }} />
    </div>
  );
}

export function Tabs<T extends string>({ tabs, value, onChange }: { tabs: Array<{ id: T; label: string }>; value: T; onChange: (v: T) => void }) {
  return (
    <div role="tablist" className="mb-4 flex gap-1 border-b border-line">
      {tabs.map((t) => (
        <button key={t.id} role="tab" type="button" aria-selected={value === t.id} onClick={() => onChange(t.id)}
          className={cx("-mb-px border-b-2 px-3 py-2 font-medium", value === t.id ? "border-fg text-fg" : "border-transparent text-fg2 hover:text-fg")}>
          {t.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- Tabel ---------- */
export function Table({ children }: { children: ReactNode }) {
  return <div className="overflow-x-auto"><table className="w-full border-collapse text-left">{children}</table></div>;
}
export const Th = ({ children, className }: { children?: ReactNode; className?: string }) => (
  <th className={cx("whitespace-nowrap border-b border-line bg-sidebar px-3 py-2 text-xs font-semibold text-fg2", className)}>{children}</th>
);
export const Td = ({ children, className, title }: { children?: ReactNode; className?: string; title?: string }) => (
  <td title={title} className={cx("border-b border-line px-3 py-2 align-middle", className)}>{children}</td>
);
export const Tr = ({ children, onClick }: { children: ReactNode; onClick?: () => void }) => (
  <tr onClick={onClick} className="hover:bg-hover">{children}</tr>
);

/* ---------- Modal ---------- */
export function Modal({ title, onClose, children, footer, wide }: { title: string; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("input,select,textarea,button:not([data-close])");
    (first ?? ref.current)?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { e.stopPropagation(); onClose(); }
      if (e.key === "Tab" && ref.current) {
        const els = [...ref.current.querySelectorAll<HTMLElement>("a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex='-1'])")];
        if (!els.length) return;
        const a = els[0]!, z = els[els.length - 1]!;
        if (e.shiftKey && document.activeElement === a) { e.preventDefault(); z.focus(); }
        else if (!e.shiftKey && document.activeElement === z) { e.preventDefault(); a.focus(); }
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => { document.removeEventListener("keydown", onKey, true); prev?.focus?.(); };
  }, [onClose]);
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}
        className={cx("flex max-h-[88vh] w-full flex-col rounded-md border border-line bg-card shadow-sm", wide ? "max-w-3xl" : "max-w-lg")}>
        <header className="flex items-center justify-between border-b border-line px-5 py-3">
          <h2 id={titleId} className="text-[15px] font-semibold">{title}</h2>
          <button data-close type="button" onClick={onClose} aria-label="Tutup" className="rounded-md px-2 py-0.5 text-fg2 hover:bg-hover">✕</button>
        </header>
        <div className="overflow-y-auto px-5 py-4">{children}</div>
        {footer && <footer className="flex justify-end gap-2 border-t border-line px-5 py-3">{footer}</footer>}
      </div>
    </div>,
    document.body,
  );
}

export function ConfirmDialog({
  title, message, confirmLabel = "Lanjutkan", destructive, onConfirm, onCancel, children, requireText,
}: {
  title: string; message: ReactNode; confirmLabel?: string; destructive?: boolean;
  onConfirm: () => void | Promise<void>; onCancel: () => void; children?: ReactNode;
  /** Pengguna harus mengetik teks ini agar tombol aktif. */
  requireText?: string;
}) {
  const [busy, setBusy] = useState(false);
  const [typed, setTyped] = useState("");
  const ok = !requireText || typed === requireText;
  return (
    <Modal title={title} onClose={onCancel}
      footer={<>
        <Button onClick={onCancel}>Batal</Button>
        <Button variant={destructive ? "danger" : "primary"} loading={busy} disabled={!ok}
          onClick={async () => { setBusy(true); try { await onConfirm(); } finally { setBusy(false); } }}>
          {destructive && <AlertTriangle className="size-4" aria-hidden />}{confirmLabel}
        </Button>
      </>}>
      <div className="space-y-3">
        <div>{message}</div>
        {children}
        {requireText && (
          <Field label={`Ketik "${requireText}" untuk mengonfirmasi`}>
            {(id) => <Input id={id} value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" />}
          </Field>
        )}
      </div>
    </Modal>
  );
}
