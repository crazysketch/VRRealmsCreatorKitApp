import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, CheckCircle2, XCircle } from "lucide-react";

export type CheckState = "ok" | "bad" | "warn";

export default function CheckRow({
  state,
  title,
  text,
  children,
}: {
  state: CheckState;
  title: string;
  text: ReactNode;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  const Icon = state === "ok" ? CheckCircle2 : state === "bad" ? XCircle : AlertTriangle;
  const badge =
    state === "ok" ? (
      <span className="badge badge-success">{t("check.ready")}</span>
    ) : state === "bad" ? (
      <span className="badge badge-danger">{t("check.missing")}</span>
    ) : (
      <span className="badge badge-warning">{t("check.action")}</span>
    );
  return (
    <div className="check-row">
      <Icon className={`check-icon ${state}`} />
      <div className="check-body">
        <div className="check-title">
          {title}
          {badge}
        </div>
        <div className="check-text">{text}</div>
        {children && <div className="actions">{children}</div>}
      </div>
    </div>
  );
}
