import type { ReactNode } from "react";
import "./styles.css";

export interface NavigationItem {
  id: string;
  href: string;
  label: string;
  current?: boolean;
}

export interface AppShellProps {
  productName: string;
  navigation: readonly NavigationItem[];
  accountSlot?: ReactNode;
  children: ReactNode;
}

export function AppShell({
  productName,
  navigation,
  accountSlot,
  children,
}: AppShellProps) {
  return (
    <div className="crm-shell">
      <a className="crm-skip-link" href="#main-content">
        Skip to main content
      </a>
      <header className="crm-topbar">
        <div className="crm-brand" aria-label={productName}>
          <span className="crm-brand-mark" aria-hidden="true">
            U
          </span>
          <span>{productName}</span>
        </div>
        <div className="crm-account-slot">{accountSlot}</div>
      </header>
      <div className="crm-shell-body">
        <nav className="crm-navigation" aria-label="Primary navigation">
          <ul>
            {navigation.map((item) => (
              <li key={item.id}>
                <a
                  className={item.current ? "crm-nav-link is-current" : "crm-nav-link"}
                  href={item.href}
                  aria-current={item.current ? "page" : undefined}
                >
                  {item.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>
        <main className="crm-main" id="main-content" tabIndex={-1}>
          {children}
        </main>
      </div>
    </div>
  );
}

export interface PageHeaderProps {
  eyebrow?: string;
  title: string;
  description?: string;
  actions?: ReactNode;
}

export function PageHeader({ eyebrow, title, description, actions }: PageHeaderProps) {
  return (
    <header className="crm-page-header">
      <div>
        {eyebrow ? <p className="crm-eyebrow">{eyebrow}</p> : null}
        <h1>{title}</h1>
        {description ? <p className="crm-page-description">{description}</p> : null}
      </div>
      {actions ? <div className="crm-page-actions">{actions}</div> : null}
    </header>
  );
}
