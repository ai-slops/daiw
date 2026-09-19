import type { ButtonHTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly variant?: "ghost" | "subtle" | "primary" | "danger";
  readonly icon?: LucideIcon;
};

export const Button = ({ variant = "subtle", icon: Icon, className = "", children, ...props }: ButtonProps) => (
  <button className={`button button--${variant} ${className}`} type="button" {...props}>
    {Icon === undefined ? null : <Icon aria-hidden="true" size="var(--icon-sm)" strokeWidth={1.8} />}
    {children}
  </button>
);

type IconButtonProps = Omit<ButtonProps, "children"> & {
  readonly label: string;
  readonly icon: LucideIcon;
};

export const IconButton = ({ label, icon: Icon, ...props }: IconButtonProps) => (
  <Button {...props} className={`icon-button ${props.className ?? ""}`} aria-label={label} title={label}>
    <Icon aria-hidden="true" size="var(--icon-md)" strokeWidth={1.7} />
  </Button>
);

type PanelProps = {
  readonly title: string;
  readonly eyebrow?: string;
  readonly actions?: ReactNode;
  readonly children: ReactNode;
  readonly className?: string;
};

export const Panel = ({ title, eyebrow, actions, children, className = "" }: PanelProps) => (
  <section className={`panel ${className}`} aria-label={title}>
    <header className="panel__header">
      <div className="panel__heading">
        {eyebrow === undefined ? null : <span className="eyebrow">{eyebrow}</span>}
        <h2>{title}</h2>
      </div>
      {actions === undefined ? null : <div className="cluster">{actions}</div>}
    </header>
    <div className="panel__body">{children}</div>
  </section>
);

type TabsProps<T extends string> = {
  readonly value: T;
  readonly items: readonly { readonly value: T; readonly label: string }[];
  readonly onChange: (value: T) => void;
  readonly label: string;
};

export const Tabs = <T extends string>({ value, items, onChange, label }: TabsProps<T>) => (
  <div className="tabs" role="tablist" aria-label={label}>
    {items.map((item) => (
      <button
        key={item.value}
        type="button"
        role="tab"
        aria-selected={item.value === value}
        className="tabs__item"
        onClick={() => onChange(item.value)}
      >
        {item.label}
      </button>
    ))}
  </div>
);

export const EmptyState = ({ children }: { readonly children: ReactNode }) => (
  <div className="empty-state">{children}</div>
);
